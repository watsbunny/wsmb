//! Coarse-to-fine dense motion estimation.
//!
//! The estimator is a predictor-driven block matcher: a Gaussian-free box
//! pyramid is built for both frames, the coarsest level is searched exhaustively
//! inside a bounded window, and every finer level only searches a +-1 window
//! around the upsampled prediction. A 3x3 median filter is applied at each level
//! to remove impulse-like outliers.
//!
//! This is deliberately a classic, well documented approach (block matching +
//! pyramidal prediction + median regularisation). It is not derived from any
//! particular commercial implementation.

use crate::frame::Frame;
use crate::par::rows_out;

/// Dense displacement field, interleaved `(u, v)` per pixel.
#[derive(Clone, Debug)]
pub struct FlowField {
    pub width: usize,
    pub height: usize,
    pub data: Vec<f32>,
}

impl FlowField {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![0.0; width * height * 2],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        (y * self.width + x) * 2
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> (f32, f32) {
        let i = self.idx(x, y);
        (self.data[i], self.data[i + 1])
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, u: f32, v: f32) {
        let i = self.idx(x, y);
        self.data[i] = u;
        self.data[i + 1] = v;
    }

    /// Bilinear sample with edge clamping.
    pub fn sample(&self, x: f32, y: f32) -> (f32, f32) {
        if self.width == 0 || self.height == 0 {
            return (0.0, 0.0);
        }
        let x = if x.is_finite() { x } else { 0.0 };
        let y = if y.is_finite() { y } else { 0.0 };
        let x = x.clamp(0.0, (self.width - 1) as f32);
        let y = y.clamp(0.0, (self.height - 1) as f32);

        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(self.width - 1);
        let y1 = (y0 + 1).min(self.height - 1);
        let fx = x - x0 as f32;
        let fy = y - y0 as f32;

        let (a0, a1) = self.get(x0, y0);
        let (b0, b1) = self.get(x1, y0);
        let (c0, c1) = self.get(x0, y1);
        let (d0, d1) = self.get(x1, y1);

        let u = (a0 + (b0 - a0) * fx) + (((c0 + (d0 - c0) * fx)) - (a0 + (b0 - a0) * fx)) * fy;
        let v = (a1 + (b1 - a1) * fx) + (((c1 + (d1 - c1) * fx)) - (a1 + (b1 - a1) * fx)) * fy;
        (u, v)
    }

    /// Resample to another resolution and scale the vectors accordingly.
    ///
    /// A displacement measured at the old resolution covers `new / old` times as
    /// many pixels at the new one, so the vectors scale with the resolution
    /// ratio (not its inverse).
    pub fn resized(&self, width: usize, height: usize) -> FlowField {
        let mut out = FlowField::new(width, height);
        if self.width == 0 || self.height == 0 || width == 0 || height == 0 {
            return out;
        }
        let inv_x = self.width as f32 / width as f32;
        let inv_y = self.height as f32 / height as f32;
        let scale_x = width as f32 / self.width as f32;
        let scale_y = height as f32 / self.height as f32;
        for y in 0..height {
            for x in 0..width {
                let (u, v) = self.sample(
                    (x as f32 + 0.5) * inv_x - 0.5,
                    (y as f32 + 0.5) * inv_y - 0.5,
                );
                out.set(x, y, u * scale_x, v * scale_y);
            }
        }
        out
    }

    /// 3x3 median filter on each component.
    pub fn median3(&mut self) {
        let (w, h) = (self.width, self.height);
        if w == 0 || h == 0 {
            return;
        }
        let src = self.data.clone();
        let mut out = vec![0.0f32; src.len()];
        let mut buf = [0.0f32; 9];
        for y in 0..h {
            for x in 0..w {
                for c in 0..2 {
                    let mut k = 0;
                    for dy in -1i32..=1 {
                        for dx in -1i32..=1 {
                            let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                            let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                            buf[k] = src[(sy * w + sx) * 2 + c];
                            k += 1;
                        }
                    }
                    buf.sort_unstable_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                    out[(y * w + x) * 2 + c] = buf[4];
                }
            }
        }
        self.data = out;
    }

    /// Mean displacement magnitude, useful for diagnostics.
    pub fn mean_magnitude(&self) -> f32 {
        if self.data.is_empty() {
            return 0.0;
        }
        let mut sum = 0.0f64;
        for i in 0..self.width * self.height {
            let (u, v) = (self.data[i * 2], self.data[i * 2 + 1]);
            sum += ((u * u + v * v) as f64).sqrt();
        }
        (sum / (self.width * self.height) as f64) as f32
    }
}

#[derive(Clone, Debug)]
pub struct FlowOptions {
    /// Pyramid depth, 1 = single resolution.
    pub levels: usize,
    /// Search radius in pixels on the coarsest level.
    pub search_radius: i32,
    /// Block radius; 1 means a 3x3 matching window.
    pub block_radius: i32,
    /// Median filter passes per level.
    pub median_passes: u32,
}

impl Default for FlowOptions {
    fn default() -> Self {
        Self {
            levels: 4,
            search_radius: 3,
            block_radius: 1,
            median_passes: 2,
        }
    }
}

/// Small penalty per pixel of search offset. Keeps textureless regions pinned to
/// the prediction instead of letting the first candidate win an all-zero tie.
const REG_WEIGHT: f32 = 0.005;

#[inline]
fn px(buf: &[f32], w: usize, h: usize, x: i32, y: i32) -> f32 {
    let xi = x.clamp(0, w as i32 - 1) as usize;
    let yi = y.clamp(0, h as i32 - 1) as usize;
    buf[yi * w + xi]
}

#[inline]
fn sad(
    src: &[f32],
    dst: &[f32],
    w: usize,
    h: usize,
    x: i32,
    y: i32,
    du: i32,
    dv: i32,
    block: i32,
) -> f32 {
    let mut sum = 0.0f32;
    for by in -block..=block {
        for bx in -block..=block {
            let a = px(src, w, h, x + bx, y + by);
            let b = px(dst, w, h, x + bx + du, y + by + dv);
            sum += (a - b).abs();
        }
    }
    sum
}

fn block_match(
    src: &[f32],
    dst: &[f32],
    w: usize,
    h: usize,
    pred: Option<&FlowField>,
    radius: i32,
    block: i32,
) -> FlowField {
    let data = rows_out(w, 2, h, |y, row| {
        for x in 0..w {
            let (pu, pv) = match pred {
                Some(p) => p.get(x, y),
                None => (0.0, 0.0),
            };
            let base_u = pu.round() as i32;
            let base_v = pv.round() as i32;

            let mut best = f32::INFINITY;
            let mut best_u = pu;
            let mut best_v = pv;

            for dv in -radius..=radius {
                for du in -radius..=radius {
                    let cost = sad(
                        src,
                        dst,
                        w,
                        h,
                        x as i32,
                        y as i32,
                        base_u + du,
                        base_v + dv,
                        block,
                    ) + REG_WEIGHT * (du * du + dv * dv) as f32;
                    if cost < best {
                        best = cost;
                        // keep the sub-pixel part of the prediction
                        best_u = (base_u + du) as f32 + (pu - pu.round());
                        best_v = (base_v + dv) as f32 + (pv - pv.round());
                    }
                }
            }

            row[x * 2] = best_u;
            row[x * 2 + 1] = best_v;
        }
    });

    FlowField {
        width: w,
        height: h,
        data,
    }
}

fn downscale_luma(src: &[f32], w: usize, h: usize) -> (Vec<f32>, usize, usize) {
    let nw = (w + 1) / 2;
    let nh = (h + 1) / 2;
    let mut out = vec![0.0f32; nw * nh];
    for y in 0..nh {
        let y0 = (2 * y).min(h - 1);
        let y1 = (2 * y + 1).min(h - 1);
        for x in 0..nw {
            let x0 = (2 * x).min(w - 1);
            let x1 = (2 * x + 1).min(w - 1);
            out[y * nw + x] = (src[y0 * w + x0]
                + src[y0 * w + x1]
                + src[y1 * w + x0]
                + src[y1 * w + x1])
                * 0.25;
        }
    }
    (out, nw, nh)
}

fn pyramid(f: &Frame, levels: usize) -> Vec<(Vec<f32>, usize, usize)> {
    let mut out = vec![(f.luma(), f.width, f.height)];
    for _ in 1..levels.max(1) {
        let (d, dw, dh) = {
            let last = out.last().expect("pyramid is never empty");
            if last.1 < 24 || last.2 < 24 {
                break;
            }
            downscale_luma(&last.0, last.1, last.2)
        };
        out.push((d, dw, dh));
    }
    out
}

/// Estimate the displacement field that maps `src` pixels onto `dst`.
pub fn dense_flow(src: &Frame, dst: &Frame, opts: &FlowOptions) -> FlowField {
    assert_eq!(
        (src.width, src.height),
        (dst.width, dst.height),
        "dense_flow needs equally sized frames"
    );
    let (w, h) = (src.width, src.height);
    if w == 0 || h == 0 {
        return FlowField::new(w, h);
    }

    let pa = pyramid(src, opts.levels);
    let pb = pyramid(dst, opts.levels);
    let n = pa.len().min(pb.len());

    let mut flow: Option<FlowField> = None;
    for level in (0..n).rev() {
        let (w, h) = (pa[level].1, pa[level].2);
        let pred = flow.as_ref().map(|f| f.resized(w, h));
        let radius = if level == n - 1 {
            opts.search_radius.max(1)
        } else {
            1
        };

        let mut f = block_match(
            &pa[level].0,
            &pb[level].0,
            w,
            h,
            pred.as_ref(),
            radius,
            opts.block_radius.max(1),
        );
        for _ in 0..opts.median_passes {
            f.median3();
        }
        flow = Some(f);
    }

    let f = flow.expect("pyramid always has at least one level");
    if f.width != src.width || f.height != src.height {
        f.resized(src.width, src.height)
    } else {
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translate(src: &Frame, dx: i32, dy: i32) -> Frame {
        let mut out = Frame::new(src.width, src.height, src.channels);
        for y in 0..src.height {
            for x in 0..src.width {
                let sx = (x as i32 - dx).clamp(0, src.width as i32 - 1) as usize;
                let sy = (y as i32 - dy).clamp(0, src.height as i32 - 1) as usize;
                for c in 0..src.channels {
                    out.set(x, y, c, src.at(sx, sy, c));
                }
            }
        }
        out
    }

    /// Smooth, aperiodic value noise. A checkerboard would be a degenerate
    /// target for block matching because every 8 px shift matches equally well.
    fn smooth_noise(w: usize, h: usize) -> Frame {
        let cell = 8.0f32;
        let gw = (w as f32 / cell).ceil() as usize + 3;
        let gh = (h as f32 / cell).ceil() as usize + 3;
        let mut grid = vec![0.0f32; gw * gh];
        let mut seed = 0x1234_5678u32;
        for v in grid.iter_mut() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *v = (seed >> 8) as f32 / 16_777_215.0;
        }

        let mut f = Frame::new(w, h, 1);
        for y in 0..h {
            for x in 0..w {
                let gx = x as f32 / cell;
                let gy = y as f32 / cell;
                let x0 = gx.floor() as usize;
                let y0 = gy.floor() as usize;
                let fx = gx - x0 as f32;
                let fy = gy - y0 as f32;
                let at = |ix: usize, iy: usize| grid[(iy * gw + ix).min(grid.len() - 1)];
                let top = at(x0, y0) + (at(x0 + 1, y0) - at(x0, y0)) * fx;
                let bot = at(x0, y0 + 1) + (at(x0 + 1, y0 + 1) - at(x0, y0 + 1)) * fx;
                f.set(x, y, 0, top + (bot - top) * fy);
            }
        }
        f
    }

    #[test]
    fn recovers_pure_translation() {
        let a = smooth_noise(64, 64);
        let b = translate(&a, 3, -2);
        let opts = FlowOptions {
            levels: 2,
            search_radius: 5,
            block_radius: 1,
            median_passes: 1,
        };
        let f = dense_flow(&a, &b, &opts);

        for (x, y) in [(32, 32), (24, 40), (40, 24)] {
            let (u, v) = f.get(x, y);
            assert!((u - 3.0).abs() <= 0.5, "u = {u} at ({x},{y})");
            assert!((v + 2.0).abs() <= 0.5, "v = {v} at ({x},{y})");
        }
    }

    #[test]
    fn zero_flow_on_identical_frames() {
        let a = smooth_noise(32, 32);
        let f = dense_flow(&a, &a, &FlowOptions::default());
        assert!(f.mean_magnitude() < 1e-3);
    }
}

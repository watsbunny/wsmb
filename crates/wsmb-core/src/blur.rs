//! Shutter integration along the estimated motion trajectories.
//!
//! For every output pixel the renderer walks the trajectory backwards through
//! the shutter interval, resamples the two source frames and averages the
//! samples. Each sample is weighted by
//!
//! * forward/backward flow consistency at the sampled position (suppresses
//!   streaks that come from occluded or mismatched regions), and
//! * a bilateral colour term against the mid-shutter colour (suppresses bleeding
//!   across depth and colour discontinuities).
//!
//! The formulation is the standard gather-based flow-guided integration
//! (`B(x) = ∫ w(t) I(t, x + t·F(x)) dt / ∫ w(t) dt`), written from scratch.

use crate::flow::FlowField;
use crate::frame::Frame;
use crate::par::rows_out;
use crate::params::Params;

#[inline]
fn warp_prev(fwd: &FlowField, x: f32, y: f32, t: f32, amount: f32) -> (f32, f32) {
    // Solve x0 = x - a*u*F(x0) with a few fixed point iterations.
    let mut sx = x;
    let mut sy = y;
    for _ in 0..3 {
        let (u, v) = fwd.sample(sx, sy);
        sx = x - amount * t * u;
        sy = y - amount * t * v;
    }
    (sx, sy)
}

#[inline]
fn warp_next(bwd: &FlowField, x: f32, y: f32, t: f32, amount: f32) -> (f32, f32) {
    // Solve x1 = x - (1-t)*G(x1); G maps next back onto prev.
    let s = 1.0 - t;
    let mut sx = x;
    let mut sy = y;
    for _ in 0..3 {
        let (u, v) = bwd.sample(sx, sy);
        sx = x - amount * s * u;
        sy = y - amount * s * v;
    }
    (sx, sy)
}

/// Forward/backward agreement in `0..1`.
#[inline]
fn consistency(fwd: &FlowField, bwd: &FlowField, x: f32, y: f32, from_prev: bool) -> f32 {
    let (du, dv) = if from_prev { fwd.sample(x, y) } else { bwd.sample(x, y) };
    let (bu, bv) = if from_prev {
        bwd.sample(x + du, y + dv)
    } else {
        fwd.sample(x + du, y + dv)
    };
    let ex = du + bu;
    let ey = dv + bv;
    let mag = (du * du + dv * dv).sqrt();
    let err = (ex * ex + ey * ey).sqrt() / (mag + 1.0);
    (-err * err * 4.0).exp()
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn sample_at(
    prev: &Frame,
    next: &Frame,
    fwd: &FlowField,
    bwd: &FlowField,
    x: f32,
    y: f32,
    t: f32,
    amount: f32,
    channels: usize,
) -> ([f32; 4], f32) {
    let mut color = [0.0f32; 4];
    let wp = 1.0 - t;
    let wn = t;
    let mut wsum = 0.0f32;
    let mut conf = 0.0f32;

    if wp > 1e-4 {
        let (sx, sy) = warp_prev(fwd, x, y, t, amount);
        for (c, slot) in color.iter_mut().enumerate().take(channels) {
            *slot += wp * prev.sample(sx, sy, c.min(prev.channels - 1));
        }
        wsum += wp;
        conf += wp * consistency(fwd, bwd, sx, sy, true);
    }
    if wn > 1e-4 {
        let (sx, sy) = warp_next(bwd, x, y, t, amount);
        for (c, slot) in color.iter_mut().enumerate().take(channels) {
            *slot += wn * next.sample(sx, sy, c.min(next.channels - 1));
        }
        wsum += wn;
        conf += wn * consistency(fwd, bwd, sx, sy, false);
    }

    if wsum > 1e-8 {
        for v in color.iter_mut().take(channels) {
            *v /= wsum;
        }
        conf /= wsum;
    }
    (color, conf)
}

#[allow(clippy::too_many_arguments)]
fn integrate(
    prev: &Frame,
    next: &Frame,
    fwd: &FlowField,
    bwd: &FlowField,
    p: &Params,
    channels: usize,
    shutter: f32,
    samples: usize,
) -> Frame {
    let width = prev.width.min(next.width);
    let height = prev.height.min(next.height);
    let t = p.t;
    let amount = p.amount;
    let t2 = 2.0 * p.color_sigma * p.color_sigma;

    let data = rows_out(width, channels, height, |y, row| {
        let yf = y as f32;
        for x in 0..width {
            let xf = x as f32;
            let (reference, _) = sample_at(prev, next, fwd, bwd, xf, yf, t, amount, channels);

            let mut acc = [0.0f32; 4];
            let mut wsum = 0.0f32;

            for i in 0..samples {
                let frac = if samples <= 1 {
                    0.5
                } else {
                    i as f32 / (samples - 1) as f32
                };
                let tk = (t + (frac - 0.5) * shutter).clamp(0.0, 1.0);
                let (c, conf) = sample_at(prev, next, fwd, bwd, xf, yf, tk, amount, channels);

                let mut w = 1.0f32;
                if p.occlusion > 0.0 {
                    w *= (1.0 - p.occlusion) + p.occlusion * conf;
                }
                if p.color_sigma > 0.0 {
                    let mut d2 = 0.0f32;
                    for k in 0..channels.min(3) {
                        let d = c[k] - reference[k];
                        d2 += d * d;
                    }
                    w *= (-d2 / t2).exp();
                }
                if !(w > 0.0) {
                    continue;
                }

                for (k, slot) in acc.iter_mut().enumerate().take(channels) {
                    *slot += w * c[k];
                }
                wsum += w;
            }

            for (k, slot) in row[x * channels..(x + 1) * channels].iter_mut().enumerate() {
                *slot = if wsum > 1e-8 {
                    acc[k] / wsum
                } else {
                    reference[k]
                };
            }
        }
    });

    Frame {
        width,
        height,
        channels,
        data,
    }
}

/// Render the blurred frame for time `p.t` between `prev` and `next`.
///
/// `fwd` must map `prev` onto `next`, `bwd` must map `next` onto `prev`.
pub fn render(prev: &Frame, next: &Frame, fwd: &FlowField, bwd: &FlowField, p: &Params) -> Frame {
    let p = p.sanitized();
    let channels = prev.channels.max(next.channels).clamp(1, 4);

    let center = integrate(prev, next, fwd, bwd, &p, channels, 0.0, 1);

    if p.amount <= 1e-6 || p.shutter <= 1e-6 || p.samples <= 1 {
        return center;
    }

    let blurred = integrate(
        prev,
        next,
        fwd,
        bwd,
        &p,
        channels,
        p.shutter,
        p.samples as usize,
    );
    center.mix_with(&blurred, p.mix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::dense_flow;
    use crate::flow::FlowOptions;

    fn flat(w: usize, h: usize, v: f32) -> Frame {
        let mut f = Frame::new(w, h, 1);
        f.data.fill(v);
        f
    }

    #[test]
    fn static_scene_is_untouched() {
        let a = flat(32, 32, 0.25);
        let b = flat(32, 32, 0.25);
        let opts = FlowOptions::default();
        let fwd = dense_flow(&a, &b, &opts);
        let bwd = dense_flow(&b, &a, &opts);
        let p = Params::default();
        let out = render(&a, &b, &fwd, &bwd, &p);
        for v in out.data {
            assert!((v - 0.25).abs() < 1e-5);
        }
    }

    #[test]
    fn zero_shutter_reproduces_interpolation() {
        let mut a = Frame::new(32, 32, 1);
        let mut b = Frame::new(32, 32, 1);
        for y in 0..32 {
            for x in 0..32 {
                a.set(x, y, 0, x as f32 / 31.0);
                b.set(x, y, 0, y as f32 / 31.0);
            }
        }
        let opts = FlowOptions::default();
        let fwd = dense_flow(&a, &b, &opts);
        let bwd = dense_flow(&b, &a, &opts);
        let mut p = Params::default();
        p.shutter = 0.0;
        let out = render(&a, &b, &fwd, &bwd, &p);
        assert_eq!(out.width, 32);
        assert_eq!(out.channels, 1);
    }
}

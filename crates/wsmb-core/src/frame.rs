//! Minimal float image container used by the whole pipeline.

#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub width: usize,
    pub height: usize,
    pub channels: usize,
    /// Interleaved, row-major, nominally in `0..1`.
    pub data: Vec<f32>,
}

impl Frame {
    pub fn new(width: usize, height: usize, channels: usize) -> Self {
        assert!(channels >= 1 && channels <= 4, "unsupported channel count");
        Self {
            width,
            height,
            channels,
            data: vec![0.0; width * height * channels],
        }
    }

    #[inline]
    pub fn idx(&self, x: usize, y: usize) -> usize {
        (y * self.width + x) * self.channels
    }

    #[inline]
    pub fn at(&self, x: usize, y: usize, c: usize) -> f32 {
        self.data[self.idx(x, y) + c]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, c: usize, v: f32) {
        let i = self.idx(x, y) + c;
        self.data[i] = v;
    }

    /// Rec.709 luminance, one value per pixel.
    pub fn luma(&self) -> Vec<f32> {
        let n = self.width * self.height;
        let mut out = vec![0.0f32; n];
        match self.channels {
            1 | 2 => {
                for (i, v) in out.iter_mut().enumerate() {
                    *v = self.data[i * self.channels];
                }
            }
            _ => {
                for (i, v) in out.iter_mut().enumerate() {
                    let b = i * self.channels;
                    *v = 0.2126 * self.data[b]
                        + 0.7152 * self.data[b + 1]
                        + 0.0722 * self.data[b + 2];
                }
            }
        }
        out
    }

    /// Bilinear sample with edge clamping. Non-finite coordinates sample (0, 0).
    pub fn sample(&self, x: f32, y: f32, c: usize) -> f32 {
        if self.width == 0 || self.height == 0 || c >= self.channels {
            return 0.0;
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

        let a = self.at(x0, y0, c);
        let b = self.at(x1, y0, c);
        let d = self.at(x0, y1, c);
        let e = self.at(x1, y1, c);

        let top = a + (b - a) * fx;
        let bot = d + (e - d) * fx;
        top + (bot - top) * fy
    }

    /// 2x box downscale; odd sizes clamp the last row/column.
    pub fn downscale2(&self) -> Frame {
        if self.width <= 1 && self.height <= 1 {
            return self.clone();
        }
        let w = (self.width + 1) / 2;
        let h = (self.height + 1) / 2;
        let mut out = Frame::new(w, h, self.channels);
        for y in 0..h {
            let y0 = (2 * y).min(self.height - 1);
            let y1 = (2 * y + 1).min(self.height - 1);
            for x in 0..w {
                let x0 = (2 * x).min(self.width - 1);
                let x1 = (2 * x + 1).min(self.width - 1);
                for c in 0..self.channels {
                    let s = self.at(x0, y0, c)
                        + self.at(x1, y0, c)
                        + self.at(x0, y1, c)
                        + self.at(x1, y1, c);
                    out.set(x, y, c, s * 0.25);
                }
            }
        }
        out
    }

    pub fn from_rgba8(width: usize, height: usize, px: &[u8]) -> Frame {
        assert_eq!(px.len(), width * height * 4, "rgba8 buffer size mismatch");
        let mut f = Frame::new(width, height, 4);
        for i in 0..width * height {
            for c in 0..4 {
                f.data[i * 4 + c] = px[i * 4 + c] as f32 / 255.0;
            }
        }
        f
    }

    pub fn to_rgba8(&self) -> Vec<u8> {
        let n = self.width * self.height;
        let mut out = vec![255u8; n * 4];
        for i in 0..n {
            for c in 0..4 {
                let src = self.data[i * self.channels + c.min(self.channels - 1)];
                out[i * 4 + c] = (src.clamp(0.0, 1.0) * 255.0 + 0.5) as u8;
            }
        }
        out
    }

    /// Linear mix of two same-sized frames.
    pub fn mix_with(&self, other: &Frame, t: f32) -> Frame {
        assert_eq!((self.width, self.height), (other.width, other.height));
        let t = t.clamp(0.0, 1.0);
        let mut out = self.clone();
        for (i, v) in out.data.iter_mut().enumerate() {
            *v = self.data[i] + (other.data[i] - self.data[i]) * t;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bilinear_hits_pixel_centres() {
        let mut f = Frame::new(4, 4, 1);
        f.set(1, 2, 0, 1.0);
        assert_eq!(f.sample(1.0, 2.0, 0), 1.0);
        assert_eq!(f.sample(0.0, 2.0, 0), 0.0);
    }

    #[test]
    fn bilinear_averages_neighbours() {
        let mut f = Frame::new(2, 2, 1);
        f.set(0, 0, 0, 0.0);
        f.set(1, 0, 0, 1.0);
        f.set(0, 1, 0, 1.0);
        f.set(1, 1, 0, 1.0);
        let v = f.sample(0.5, 0.5, 0);
        assert!((v - 0.75).abs() < 1e-6);
    }
}

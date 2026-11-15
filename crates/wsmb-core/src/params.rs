use crate::flow::FlowOptions;

/// User facing controls. Everything here is mean to map 1:1 onto OFX params.
#[derive(Clone, Debug)]
pub struct Params {
    /// Render time inside the `prev..next` interval, normally `0.5`.
    pub t: f32,
    /// Shutter duration as a fraction of the `prev..next` interval.
    /// `0.5` means the shutter is open for half a frame period.
    pub shutter: f32,
    /// Number of integration samples inside the shutter interval.
    pub samples: u32,
    /// Global multiplier applied to the estimated motion.
    pub amount: f32,
    /// Motion multiplier for the area the matte marks as foreground.
    pub fg_amount: f32,
    /// Blend of the blurred result with the plain interpolated frame, `0..1`.
    pub mix: f32,
    /// Weight of the forward/backward consistency term, `0..1`.
    pub occlusion: f32,
    /// Colour sensitivity of the bilateral term; `0` disables it.
    pub color_sigma: f32,
    /// Motion estimation settings.
    pub flow: FlowOptions,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            t: 0.5,
            shutter: 0.5,
            samples: 16,
            amount: 1.0,
            fg_amount: 1.0,
            mix: 1.0,
            occlusion: 0.8,
            color_sigma: 0.15,
            flow: FlowOptions::default(),
        }
    }
}

impl Params {
    /// Clamp everything into a renderable range.
    pub fn sanitized(&self) -> Params {
        let mut p = self.clone();
        p.t = p.t.clamp(0.0, 1.0);
        p.shutter = p.shutter.clamp(0.0, 4.0);
        p.samples = p.samples.clamp(1, 256);
        p.amount = p.amount.clamp(0.0, 8.0);
        p.fg_amount = p.fg_amount.clamp(0.0, 8.0);
        p.mix = p.mix.clamp(0.0, 1.0);
        p.occlusion = p.occlusion.clamp(0.0, 1.0);
        p.color_sigma = p.color_sigma.max(0.0);
        p.flow.levels = p.flow.levels.clamp(1, 8);
        p.flow.search_radius = p.flow.search_radius.clamp(1, 32);
        p.flow.block_radius = p.flow.block_radius.clamp(1, 8);
        p.flow.median_passes = p.flow.median_passes.min(8);
        p.flow.refine_passes = p.flow.refine_passes.clamp(1, 8);
        p.flow.smooth_passes = p.flow.smooth_passes.min(8);
        p
    }
}

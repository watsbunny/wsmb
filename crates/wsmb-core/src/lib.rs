//! WSMB core — motion estimation and motion blur synthesis.
//!
//! The crate is deliberately dependency free and backend agnostic. It works on
//! plain `f32` frames, so the same code can be driven by the OFX plugin, by the
//! dev CLI, or by a GPU backend that uploads the resulting buffers.
//!
//! Pipeline:
//!
//! 1. [`dense_flow`] estimates dense forward and backward displacement fields
//!    between two neighbouring frames with a coarse-to-fine block matcher.
//! 2. [`render`] integrates the image along the motion trajectories inside the
//!    shutter interval, weighting samples by forward/backward consistency and by
//!    a colour (bilateral) term that suppresses bleeding across occlusions.

pub mod blur;
pub mod flow;
pub mod frame;
pub mod par;
pub mod params;

pub use blur::render;
pub use flow::{dense_flow, FlowField, FlowOptions};
pub use frame::Frame;
pub use params::Params;

/// Estimated motion between two frames, both directions.
#[derive(Clone, Debug)]
pub struct Motion {
    /// Displacement from the previous frame to the next one (`next - prev`).
    pub forward: FlowField,
    /// Displacement from the next frame to the previous one (`prev - next`).
    pub backward: FlowField,
}

/// Estimate both displacement directions for a frame pair.
pub fn estimate(prev: &Frame, next: &Frame, opts: &FlowOptions) -> Motion {
    Motion {
        forward: dense_flow(prev, next, opts),
        backward: dense_flow(next, prev, opts),
    }
}

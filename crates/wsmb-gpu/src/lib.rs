//! GPU backend: the shutter integration runs as a compute shader.
//!
//! Motion estimation stays on the CPU for now — it is a small part of the frame
//! time at typical settings and its pyramidal search maps badly onto a single
//! dispatch. The integration below is the 24-samples-per-pixel loop that
//! dominates, and it is a straight port of `wsmb_core::blur`.
//!
//! Everything degrades gracefully: if no adapter is available, `global()`
//! returns `None` and the caller keeps the CPU path.

use std::sync::mpsc;
use std::sync::OnceLock;

use wgpu::util::DeviceExt;
use wsmb_core::{FlowField, Frame, Params};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuParams {
    /// width, height, channels, samples
    size: [f32; 4],
    /// t, shutter, amount, occlusion
    timing: [f32; 4],
    /// color_sigma, - , - , -
    extra: [f32; 4],
}

pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

static GPU: OnceLock<Option<Gpu>> = OnceLock::new();

/// Lazily created process-wide device. `None` when there is no usable adapter.
pub fn global() -> Option<&'static Gpu> {
    GPU.get_or_init(Gpu::new).as_ref()
}

impl Gpu {
    fn new() -> Option<Gpu> {
        pollster::block_on(Self::create())
    }

    async fn create() -> Option<Gpu> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: None,
                apply_limit_buckets: false,
            })
            .await
            .ok()?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("wsmb"),
                ..Default::default()
            })
            .await
            .ok()?;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("wsmb blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blur.wgsl").into()),
        });

        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let uniform = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("wsmb blur"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, true),
                storage(3, true),
                storage(4, false),
                uniform(5),
                storage(6, true),
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("wsmb blur"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("wsmb blur"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Some(Gpu {
            device,
            queue,
            pipeline,
            layout,
        })
    }

    /// Same contract as `wsmb_core::render_with`, GPU resident.
    pub fn render(
        &self,
        prev: &Frame,
        next: &Frame,
        fwd: &FlowField,
        bwd: &FlowField,
        params: &Params,
        matte: Option<&[f32]>,
    ) -> Option<Frame> {
        let p = params.sanitized();
        let w = prev.width.min(next.width);
        let h = prev.height.min(next.height);
        if w == 0 || h == 0 {
            return None;
        }
        let pixels = w * h;

        let prev4 = to_rgba(prev, w, h)?;
        let next4 = to_rgba(next, w, h)?;

        let prev_buf = self.storage_buffer("prev", &prev4.data);
        let next_buf = self.storage_buffer("next", &next4.data);
        let fwd_buf = self.storage_buffer("fwd", &fwd.data);
        let bwd_buf = self.storage_buffer("bwd", &bwd.data);

        let out_size = (pixels * 16) as u64;
        let out_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("out"),
            size: out_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: out_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniforms = GpuParams {
            size: [w as f32, h as f32, 4.0, p.samples as f32],
            timing: [p.t, p.shutter, p.amount, p.occlusion],
            extra: [
                p.color_sigma,
                if matte.is_some() { 1.0 } else { 0.0 },
                p.fg_amount,
                0.0,
            ],
        };
        let uniform_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("params"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let matte_data = matte.unwrap_or(&[0.0f32][..]);
        let matte_buf = self.storage_buffer("matte", matte_data);

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("wsmb blur"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: prev_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: next_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: fwd_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: bwd_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: out_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 6,
                    resource: matte_buf.as_entire_binding(),
                },
            ],
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("wsmb blur"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.dispatch_workgroups(
                (w as u32).div_ceil(8),
                (h as u32).div_ceil(8),
                1,
            );
        }
        encoder.copy_buffer_to_buffer(&out_buf, 0, &readback, 0, out_size);
        self.queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        rx.recv().ok()?.ok()?;

        let data = slice.get_mapped_range().ok()?;
        let pixels_out: &[[f32; 4]] = bytemuck::cast_slice(&data);
        let mut frame = Frame::new(w, h, 4);
        for (i, px) in pixels_out.iter().enumerate().take(pixels) {
            frame.data[i * 4..i * 4 + 4].copy_from_slice(px);
        }
        drop(data);
        readback.unmap();

        Some(frame)
    }

    fn storage_buffer(&self, label: &str, data: &[f32]) -> wgpu::Buffer {
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(data),
                usage: wgpu::BufferUsages::STORAGE,
            })
    }
}

/// Frames are always RGBA on the GPU; expand or pad when needed.
fn to_rgba(f: &Frame, w: usize, h: usize) -> Option<Frame> {
    if f.width < w || f.height < h {
        return None;
    }
    if f.channels == 4 && f.width == w && f.height == h {
        return Some(f.clone());
    }
    let mut out = Frame::new(w, h, 4);
    for y in 0..h {
        for x in 0..w {
            let src = (y * f.width + x) * f.channels;
            let dst = (y * w + x) * 4;
            for c in 0..4 {
                out.data[dst + c] = f.data[src + c.min(f.channels - 1)];
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wsmb_core::{dense_flow, render, FlowOptions};

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
        let mut f = Frame::new(w, h, 4);
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
                let v = top + (bot - top) * fy;
                for c in 0..3 {
                    f.set(x, y, c, v * (0.6 + 0.2 * c as f32));
                }
                f.set(x, y, 3, 1.0);
            }
        }
        f
    }

    fn translate(src: &Frame, dx: usize) -> Frame {
        let mut out = Frame::new(src.width, src.height, 4);
        for y in 0..src.height {
            for x in 0..src.width {
                let sx = (x + dx).min(src.width - 1);
                for c in 0..4 {
                    out.set(x, y, c, src.at(sx, y, c));
                }
            }
        }
        out
    }

    #[test]
    fn gpu_matches_cpu() {
        let Some(gpu) = global() else {
            eprintln!("no wgpu adapter available, skipping parity test");
            return;
        };

        let a = smooth_noise(96, 96);
        let b = translate(&a, 5);
        let opts = FlowOptions {
            levels: 3,
            search_radius: 6,
            block_radius: 1,
            median_passes: 1,
        };
        let forward = dense_flow(&a, &b, &opts);
        let backward = dense_flow(&b, &a, &opts);

        let p = Params {
            t: 0.5,
            shutter: 0.9,
            samples: 16,
            amount: 1.0,
            fg_amount: 1.0,
            mix: 1.0,
            occlusion: 0.8,
            color_sigma: 0.15,
            flow: opts,
        };

        let cpu = render(&a, &b, &forward, &backward, &p);
        let gpu_out = gpu.render(&a, &b, &forward, &backward, &p, None).expect("gpu render");

        assert_eq!((cpu.width, cpu.height), (gpu_out.width, gpu_out.height));

        let mut max_diff = 0.0f32;
        let mut sum = 0.0f64;
        for (x, y) in cpu.data.iter().zip(gpu_out.data.iter()) {
            let d = (x - y).abs();
            max_diff = max_diff.max(d);
            sum += d as f64;
        }
        let mean = sum / cpu.data.len() as f64;
        eprintln!("gpu vs cpu: max {max_diff:.6}, mean {mean:.8}");
        assert!(max_diff < 2e-3, "gpu and cpu disagree: max {max_diff}");
    }
}

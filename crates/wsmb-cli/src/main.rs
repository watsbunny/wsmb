//! Dev CLI: render motion blur between two frames.
//!
//! This exists so the core can be exercised and timed without an OFX host.

mod bmp;

use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

use wsmb_core::{dense_flow, render, Frame, Params};

fn usage() -> ! {
    eprintln!(
        "\
wsmb — Watsbunny Smart Motion Blur (dev CLI)

USAGE:
    wsmb <prev.bmp> <next.bmp> <out.bmp> [options]

OPTIONS:
    --shutter <f>      shutter duration, fraction of the frame interval  [0.5]
    --samples <n>      samples inside the shutter interval               [16]
    --amount <f>       motion multiplier                                 [1.0]
    --t <f>            render time between the two frames                [0.5]
    --mix <f>          blend with the unblurred frame                    [1.0]
    --occlusion <f>    forward/backward consistency weight               [0.8]
    --sigma <f>        bilateral colour sigma, 0 disables                [0.15]
    --gpu              run the shutter integration on the GPU
    --levels <n>       pyramid levels                                    [4]
    --search <n>       block match radius on the coarsest level          [3]
    --block <n>        block radius                                      [1]
    -h, --help         show this help
"
    );
    std::process::exit(2);
}

fn next_val(it: &mut impl Iterator<Item = String>, flag: &str) -> String {
    it.next().unwrap_or_else(|| {
        eprintln!("missing value for {flag}");
        std::process::exit(2);
    })
}

fn next_f32(it: &mut impl Iterator<Item = String>, flag: &str) -> f32 {
    let v = next_val(it, flag);
    v.parse().unwrap_or_else(|_| {
        eprintln!("bad number for {flag}: {v}");
        std::process::exit(2);
    })
}

fn next_usize(it: &mut impl Iterator<Item = String>, flag: &str) -> usize {
    let v = next_val(it, flag);
    v.parse().unwrap_or_else(|_| {
        eprintln!("bad integer for {flag}: {v}");
        std::process::exit(2);
    })
}

fn next_i32(it: &mut impl Iterator<Item = String>, flag: &str) -> i32 {
    let v = next_val(it, flag);
    v.parse().unwrap_or_else(|_| {
        eprintln!("bad integer for {flag}: {v}");
        std::process::exit(2);
    })
}

fn run() -> Result<(), String> {
    let mut params = Params::default();
    let mut positional: Vec<String> = Vec::new();
    let mut use_gpu = false;

    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--gpu" => use_gpu = true,
            "--shutter" => params.shutter = next_f32(&mut it, "--shutter"),
            "--samples" => params.samples = next_usize(&mut it, "--samples") as u32,
            "--amount" => params.amount = next_f32(&mut it, "--amount"),
            "--t" => params.t = next_f32(&mut it, "--t"),
            "--mix" => params.mix = next_f32(&mut it, "--mix"),
            "--occlusion" => params.occlusion = next_f32(&mut it, "--occlusion"),
            "--sigma" => params.color_sigma = next_f32(&mut it, "--sigma"),
            "--levels" => params.flow.levels = next_usize(&mut it, "--levels"),
            "--search" => params.flow.search_radius = next_i32(&mut it, "--search"),
            "--block" => params.flow.block_radius = next_i32(&mut it, "--block"),
            "-h" | "--help" => usage(),
            other if other.starts_with('-') => {
                return Err(format!("unknown option {other}"));
            }
            other => positional.push(other.to_string()),
        }
    }

    if positional.len() != 3 {
        usage();
    }

    let prev_bmp = bmp::read(Path::new(&positional[0])).map_err(|e| format!("prev: {e}"))?;
    let next_bmp = bmp::read(Path::new(&positional[1])).map_err(|e| format!("next: {e}"))?;
    if (prev_bmp.width, prev_bmp.height) != (next_bmp.width, next_bmp.height) {
        return Err(format!(
            "frame sizes differ: {}x{} vs {}x{}",
            prev_bmp.width, prev_bmp.height, next_bmp.width, next_bmp.height
        ));
    }

    let prev = Frame::from_rgba8(prev_bmp.width, prev_bmp.height, &prev_bmp.rgba);
    let next = Frame::from_rgba8(next_bmp.width, next_bmp.height, &next_bmp.rgba);

    println!(
        "WSMB {}x{}, {} levels, {} search, {} samples",
        prev.width, prev.height, params.flow.levels, params.flow.search_radius, params.samples
    );

    let t0 = Instant::now();
    let fwd = dense_flow(&prev, &next, &params.flow);
    let t1 = Instant::now();
    let bwd = dense_flow(&next, &prev, &params.flow);
    let t2 = Instant::now();
    let out = if use_gpu {
        match wsmb_gpu::global() {
            Some(gpu) => gpu
                .render(&prev, &next, &fwd, &bwd, &params, None)
                .unwrap_or_else(|| render(&prev, &next, &fwd, &bwd, &params)),
            None => {
                eprintln!("no GPU adapter, falling back to CPU");
                render(&prev, &next, &fwd, &bwd, &params)
            }
        }
    } else {
        render(&prev, &next, &fwd, &bwd, &params)
    };
    let t3 = Instant::now();

    println!("flow fwd: {:>8.1} ms", (t1 - t0).as_secs_f64() * 1e3);
    println!("flow bwd: {:>8.1} ms", (t2 - t1).as_secs_f64() * 1e3);
    println!("render  : {:>8.1} ms", (t3 - t2).as_secs_f64() * 1e3);
    println!("mean motion: {:.2} px", fwd.mean_magnitude());

    bmp::write(Path::new(&positional[2]), out.width, out.height, &out.to_rgba8())
        .map_err(|e| format!("write: {e}"))?;
    println!("wrote {}", positional[2]);
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

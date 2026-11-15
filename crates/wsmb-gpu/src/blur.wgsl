// Shutter integration on the GPU. Mirrors crates/wsmb-core/src/blur.rs exactly:
// the same fixed-point warp, the same weights, so CPU and GPU agree.

struct Params {
    // width, height, channels, samples
    size: vec4<f32>,
    // t, shutter, amount, occlusion
    timing: vec4<f32>,
    // color_sigma, has_matte, fg_amount, -
    extra: vec4<f32>,
};

@group(0) @binding(0) var<storage, read> prev_px: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read> next_px: array<vec4<f32>>;
@group(0) @binding(2) var<storage, read> fwd: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read> bwd: array<vec2<f32>>;
@group(0) @binding(4) var<storage, read_write> out_px: array<vec4<f32>>;
@group(0) @binding(5) var<uniform> params: Params;
@group(0) @binding(6) var<storage, read> matte: array<f32>;

fn at(x: u32, y: u32, w: u32) -> u32 {
    return y * w + x;
}

fn sample_prev(x: f32, y: f32, w: u32, h: u32) -> vec4<f32> {
    let cx = clamp(x, 0.0, f32(w - 1u));
    let cy = clamp(y, 0.0, f32(h - 1u));
    let x0 = u32(floor(cx));
    let y0 = u32(floor(cy));
    let x1 = min(x0 + 1u, w - 1u);
    let y1 = min(y0 + 1u, h - 1u);
    let fx = cx - f32(x0);
    let fy = cy - f32(y0);
    let a = prev_px[at(x0, y0, w)];
    let b = prev_px[at(x1, y0, w)];
    let c = prev_px[at(x0, y1, w)];
    let d = prev_px[at(x1, y1, w)];
    let top = mix(a, b, fx);
    let bot = mix(c, d, fx);
    return mix(top, bot, fy);
}

fn sample_next(x: f32, y: f32, w: u32, h: u32) -> vec4<f32> {
    let cx = clamp(x, 0.0, f32(w - 1u));
    let cy = clamp(y, 0.0, f32(h - 1u));
    let x0 = u32(floor(cx));
    let y0 = u32(floor(cy));
    let x1 = min(x0 + 1u, w - 1u);
    let y1 = min(y0 + 1u, h - 1u);
    let fx = cx - f32(x0);
    let fy = cy - f32(y0);
    let a = next_px[at(x0, y0, w)];
    let b = next_px[at(x1, y0, w)];
    let c = next_px[at(x0, y1, w)];
    let d = next_px[at(x1, y1, w)];
    let top = mix(a, b, fx);
    let bot = mix(c, d, fx);
    return mix(top, bot, fy);
}

fn sample_fwd(x: f32, y: f32, w: u32, h: u32) -> vec2<f32> {
    let cx = clamp(x, 0.0, f32(w - 1u));
    let cy = clamp(y, 0.0, f32(h - 1u));
    let x0 = u32(floor(cx));
    let y0 = u32(floor(cy));
    let x1 = min(x0 + 1u, w - 1u);
    let y1 = min(y0 + 1u, h - 1u);
    let fx = cx - f32(x0);
    let fy = cy - f32(y0);
    let a = fwd[at(x0, y0, w)];
    let b = fwd[at(x1, y0, w)];
    let c = fwd[at(x0, y1, w)];
    let d = fwd[at(x1, y1, w)];
    let top = mix(a, b, fx);
    let bot = mix(c, d, fx);
    return mix(top, bot, fy);
}

fn sample_bwd(x: f32, y: f32, w: u32, h: u32) -> vec2<f32> {
    let cx = clamp(x, 0.0, f32(w - 1u));
    let cy = clamp(y, 0.0, f32(h - 1u));
    let x0 = u32(floor(cx));
    let y0 = u32(floor(cy));
    let x1 = min(x0 + 1u, w - 1u);
    let y1 = min(y0 + 1u, h - 1u);
    let fx = cx - f32(x0);
    let fy = cy - f32(y0);
    let a = bwd[at(x0, y0, w)];
    let b = bwd[at(x1, y0, w)];
    let c = bwd[at(x0, y1, w)];
    let d = bwd[at(x1, y1, w)];
    let top = mix(a, b, fx);
    let bot = mix(c, d, fx);
    return mix(top, bot, fy);
}

fn consistency(du: vec2<f32>, back: vec2<f32>) -> f32 {
    let e = du + back;
    let mag = length(du);
    let err = length(e) / (mag + 1.0);
    return exp(-err * err * 4.0);
}

struct Sample {
    color: vec4<f32>,
    confidence: f32,
};

fn sample_at(x: f32, y: f32, t: f32, amount: f32, w: u32, h: u32) -> Sample {
    var color = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var conf = 0.0;
    var wsum = 0.0;

    let wp = 1.0 - t;
    let wn = t;

    if (wp > 1e-4) {
        var sx = x;
        var sy = y;
        for (var i = 0; i < 3; i++) {
            let d = sample_fwd(sx, sy, w, h);
            sx = x - amount * t * d.x;
            sy = y - amount * t * d.y;
        }
        color += wp * sample_prev(sx, sy, w, h);
        wsum += wp;
        let d = sample_fwd(sx, sy, w, h);
        let back = sample_bwd(sx + d.x, sy + d.y, w, h);
        conf += wp * consistency(d, back);
    }

    if (wn > 1e-4) {
        let s = 1.0 - t;
        var sx = x;
        var sy = y;
        for (var i = 0; i < 3; i++) {
            let d = sample_bwd(sx, sy, w, h);
            sx = x - amount * s * d.x;
            sy = y - amount * s * d.y;
        }
        color += wn * sample_next(sx, sy, w, h);
        wsum += wn;
        let d = sample_bwd(sx, sy, w, h);
        let back = sample_fwd(sx + d.x, sy + d.y, w, h);
        conf += wn * consistency(d, back);
    }

    if (wsum > 1e-8) {
        color = color / wsum;
        conf = conf / wsum;
    }
    return Sample(color, conf);
}

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = u32(params.size.x);
    let h = u32(params.size.y);
    if (gid.x >= w || gid.y >= h) {
        return;
    }

    let x = f32(gid.x);
    let y = f32(gid.y);
    let t = params.timing.x;
    let shutter = params.timing.y;
    var amount = params.timing.z;
    let occlusion = params.timing.w;
    let sigma = params.extra.x;
    let fg_amount = params.extra.z;
    let samples = max(u32(params.size.w), 1u);

    // The matte picks the motion multiplier per pixel.
    if (params.extra.y > 0.5) {
        let k = clamp(matte[at(gid.x, gid.y, w)], 0.0, 1.0);
        amount = amount * (1.0 - k) + fg_amount * k;
    }

    let reference = sample_at(x, y, t, amount, w, h).color;

    var acc = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    var wsum = 0.0;
    let t2 = 2.0 * sigma * sigma;

    for (var i = 0u; i < samples; i++) {
        var frac = 0.5;
        if (samples > 1u) {
            frac = f32(i) / f32(samples - 1u);
        }
        let tk = clamp(t + (frac - 0.5) * shutter, 0.0, 1.0);
        let s = sample_at(x, y, tk, amount, w, h);

        var weight = 1.0;
        if (occlusion > 0.0) {
            weight *= (1.0 - occlusion) + occlusion * s.confidence;
        }
        if (sigma > 0.0) {
            let d = s.color.rgb - reference.rgb;
            weight *= exp(-dot(d, d) / t2);
        }
        if (weight > 0.0) {
            acc += weight * s.color;
            wsum += weight;
        }
    }

    var result = reference;
    if (wsum > 1e-8) {
        result = acc / wsum;
    }
    out_px[at(gid.x, gid.y, w)] = result;
}

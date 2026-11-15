//! The OFX image effect: descriptor, parameter set and the render action.

use crate::host::{cstr, suites, Prop};
use crate::sys::*;
use core::ffi::c_int;
use std::ffi::CString;
use std::ptr;

use wsmb_core::{dense_flow, FlowField, FlowOptions, Frame, Params};

pub const PLUGIN_LABEL: &str = "WSMB";
pub const PLUGIN_LONG_LABEL: &str = "Watsbunny Smart Motion Blur";

const P_SHUTTER: &str = "shutter";
const P_SAMPLES: &str = "samples";
const P_AMOUNT: &str = "amount";
const P_MIX: &str = "mix";
const P_OCCLUSION: &str = "occlusion";
const P_QUALITY: &str = "quality";
const P_USEGPU: &str = "usegpu";
const P_FGAMOUNT: &str = "fgamount";
const P_DISPLAY: &str = "display";
const P_MATTECH: &str = "mattechannel";
const P_MATTESHRINK: &str = "matteshrink";
const P_SENSITIVITY: &str = "sensitivity";
const P_MAXDISP: &str = "maxdisp";
const P_VECSCALEX: &str = "vecscalex";
const P_VECSCALEY: &str = "vecscaley";

const CLIP_MATTE: &str = "Matte";
const CLIP_VECTORS: &str = "Vectors";

fn cs(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Describe
// ---------------------------------------------------------------------------

pub unsafe fn describe(handle: OfxPropertySetHandle) -> OfxStatus {
    use crate::host::log;
    let Some(s) = suites() else {
        return kOfxStatErrMissingHostFeature;
    };
    // `handle` is the effect descriptor, not a property set: ask the effect
    // suite for its property set first.
    let Some(get_props) = s.effect.getPropertySet else {
        return kOfxStatErrMissingHostFeature;
    };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    let status = get_props(handle as OfxImageEffectHandle, &mut props);
    log(&format!("describe getPropertySet -> {status} {props:p}"));

    let p = Prop(props);
    p.set_string(kOfxPropType, kOfxTypeImageEffect);
    p.set_string(kOfxPropLabel, PLUGIN_LABEL);
    p.set_string(kOfxPropShortLabel, PLUGIN_LABEL);
    p.set_string(kOfxPropLongLabel, PLUGIN_LONG_LABEL);
    p.set_string(kOfxPropVersionLabel, env!("CARGO_PKG_VERSION"));
    p.set_string(kOfxImageEffectPluginPropGrouping, PLUGIN_LABEL);
    let ctx = p.set_strings(
        kOfxImageEffectPropSupportedContexts,
        &[kOfxImageEffectContextFilter, kOfxImageEffectContextGeneral],
    );
    let depth = p.set_strings(
        kOfxImageEffectPropSupportedPixelDepths,
        &[kOfxBitDepthByte, kOfxBitDepthShort, kOfxBitDepthFloat],
    );
    // We need the whole frame to estimate motion, so no tile support.
    let tiles = p.set_int(kOfxImageEffectPropSupportsTiles, 0);
    log(&format!("  contexts={ctx} depths={depth} tiles={tiles}"));
    kOfxStatOK
}

pub unsafe fn describe_in_context(
    handle: OfxImageEffectHandle,
    in_args: OfxPropertySetHandle,
) -> OfxStatus {
    let Some(s) = suites() else {
        return kOfxStatErrMissingHostFeature;
    };
    let context = Prop(in_args)
        .get_string(kOfxImageEffectPropContext)
        .unwrap_or_default();
    if context != kOfxImageEffectContextFilter && context != kOfxImageEffectContextGeneral {
        return kOfxStatErrUnsupported;
    }

    if !define_clip(s, handle, kOfxImageEffectSimpleSourceClipName, true) {
        return kOfxStatFailed;
    }
    if !define_clip(s, handle, kOfxImageEffectOutputClipName, false) {
        return kOfxStatFailed;
    }
    // Optional sources: a matte that splits foreground from background and a
    // clip of external motion vectors.
    define_optional_clip(s, handle, CLIP_MATTE);
    define_optional_clip(s, handle, CLIP_VECTORS);

    let mut param_set: OfxParamSetHandle = ptr::null_mut();
    if let Some(f) = s.effect.getParamSet {
        if f(handle, &mut param_set) != kOfxStatOK {
            return kOfxStatFailed;
        }
    }
    define_params(s, param_set);
    kOfxStatOK
}

unsafe fn define_clip(
    s: &crate::host::Suites,
    handle: OfxImageEffectHandle,
    name: &str,
    temporal: bool,
) -> bool {
    use crate::host::log;
    let Some(define) = s.effect.clipDefine else {
        return false;
    };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    let status = define(handle, cs(name).as_ptr(), &mut props);
    log(&format!("clipDefine({name}) -> {status} props={props:p}"));
    if status != kOfxStatOK {
        return false;
    }
    let p = Prop(props);
    let st_type = p.set_string(kOfxPropType, kOfxTypeClip);
    let st_comps = p.set_strings(
        kOfxImageEffectPropSupportedComponents,
        &[kOfxImageComponentRGBA, kOfxImageComponentRGB],
    );
    let st_temporal = p.set_int(
        kOfxImageEffectPropTemporalClipAccess,
        if temporal { 1 } else { 0 },
    );
    if temporal {
        p.set_int(kOfxImageClipPropOptional, 0);
    }
    log(&format!(
        "  clip {name}: type={st_type} comps={st_comps} temporal={st_temporal} readback={:?}",
        p.get_int(kOfxImageEffectPropTemporalClipAccess)
    ));
    true
}

unsafe fn define_optional_clip(s: &crate::host::Suites, handle: OfxImageEffectHandle, name: &str) {
    let Some(define) = s.effect.clipDefine else { return };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    if define(handle, cs(name).as_ptr(), &mut props) != kOfxStatOK {
        return;
    }
    let p = Prop(props);
    p.set_string(kOfxPropType, kOfxTypeClip);
    p.set_string(kOfxPropName, name);
    p.set_strings(
        kOfxImageEffectPropSupportedComponents,
        &[kOfxImageComponentRGBA, kOfxImageComponentRGB],
    );
    p.set_int(kOfxImageClipPropOptional, 1);
    p.set_int(kOfxImageEffectPropTemporalClipAccess, 0);
}

unsafe fn define_params(s: &crate::host::Suites, set: OfxParamSetHandle) {
    double_param(s, set, P_SHUTTER, "Shutter", 0.5, 0.0, 4.0, 0.01);
    int_param(s, set, P_SAMPLES, "Samples", 16, 1, 64, 1);
    double_param(s, set, P_AMOUNT, "Amount", 1.0, 0.0, 4.0, 0.01);
    double_param(s, set, P_MIX, "Mix", 1.0, 0.0, 1.0, 0.01);
    int_param(s, set, P_QUALITY, "Quality", 2, 1, 4, 1);
    bool_param(s, set, P_USEGPU, "Use GPU", 1);
    double_param(s, set, P_FGAMOUNT, "FG Amount", 1.0, 0.0, 4.0, 0.01);
    choice_param(
        s,
        set,
        P_DISPLAY,
        "Display",
        &["Source Blurred", "Motion Vectors", "Source Unblurred"],
        0,
    );
    choice_param(
        s,
        set,
        P_MATTECH,
        "Matte Channel",
        &["Luma", "Red", "Green", "Blue", "Alpha"],
        0,
    );
    double_param(s, set, P_MATTESHRINK, "Matte Shrink", 0.0, 0.0, 16.0, 0.1);
    double_param(s, set, P_SENSITIVITY, "Sensitivity", 70.0, 0.0, 100.0, 1.0);
    double_param(s, set, P_MAXDISP, "Max Displacement", 64.0, 4.0, 512.0, 1.0);
    double_param(s, set, P_VECSCALEX, "Vector Scale X", 1.0, -8.0, 8.0, 0.01);
    double_param(s, set, P_VECSCALEY, "Vector Scale Y", 1.0, -8.0, 8.0, 0.01);
}

unsafe fn double_param(
    s: &crate::host::Suites,
    set: OfxParamSetHandle,
    name: &str,
    label: &str,
    default: f64,
    min: f64,
    max: f64,
    increment: f64,
) {
    let Some(define) = s.param.paramDefine else { return };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    if define(
        set,
        cstr!("OfxParamTypeDouble"),
        cs(name).as_ptr(),
        &mut props,
    ) != kOfxStatOK
    {
        return;
    }
    let p = Prop(props);
    p.set_string(kOfxPropType, kOfxTypeParameter);
    p.set_string(kOfxParamPropType, kOfxParamTypeDouble);
    p.set_string(kOfxPropName, name);
    p.set_string(kOfxPropLabel, label);
    p.set_string(kOfxParamPropDoubleType, kOfxParamDoubleTypeScale);
    p.set_double(kOfxParamPropDefault, default);
    p.set_double(kOfxParamPropMin, min);
    p.set_double(kOfxParamPropMax, max);
    p.set_double(kOfxParamPropDisplayMin, min);
    p.set_double(kOfxParamPropDisplayMax, max);
    p.set_double(kOfxParamPropIncrement, increment);
    p.set_int(kOfxParamPropAnimates, 1);
}

unsafe fn int_param(
    s: &crate::host::Suites,
    set: OfxParamSetHandle,
    name: &str,
    label: &str,
    default: c_int,
    min: c_int,
    max: c_int,
    increment: c_int,
) {
    let Some(define) = s.param.paramDefine else { return };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    if define(
        set,
        cstr!("OfxParamTypeInteger"),
        cs(name).as_ptr(),
        &mut props,
    ) != kOfxStatOK
    {
        return;
    }
    let p = Prop(props);
    p.set_string(kOfxPropType, kOfxTypeParameter);
    p.set_string(kOfxParamPropType, kOfxParamTypeInteger);
    p.set_string(kOfxPropName, name);
    p.set_string(kOfxPropLabel, label);
    p.set_int(kOfxParamPropDefault, default);
    p.set_int(kOfxParamPropMin, min);
    p.set_int(kOfxParamPropMax, max);
    p.set_int(kOfxParamPropIncrement, increment);
    p.set_int(kOfxParamPropAnimates, 1);
}

unsafe fn choice_param(
    s: &crate::host::Suites,
    set: OfxParamSetHandle,
    name: &str,
    label: &str,
    options: &[&str],
    default: c_int,
) {
    let Some(define) = s.param.paramDefine else { return };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    if define(
        set,
        cstr!("OfxParamTypeChoice"),
        cs(name).as_ptr(),
        &mut props,
    ) != kOfxStatOK
    {
        return;
    }
    let p = Prop(props);
    p.set_string(kOfxPropType, kOfxTypeParameter);
    p.set_string(kOfxParamPropType, kOfxParamTypeChoice);
    p.set_string(kOfxPropName, name);
    p.set_string(kOfxPropLabel, label);
    p.set_strings(kOfxParamPropChoiceOption, options);
    p.set_int(kOfxParamPropDefault, default);
    p.set_int(kOfxParamPropAnimates, 0);
}

unsafe fn bool_param(
    s: &crate::host::Suites,
    set: OfxParamSetHandle,
    name: &str,
    label: &str,
    default: c_int,
) {
    let Some(define) = s.param.paramDefine else { return };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    if define(
        set,
        cstr!("OfxParamTypeBoolean"),
        cs(name).as_ptr(),
        &mut props,
    ) != kOfxStatOK
    {
        return;
    }
    let p = Prop(props);
    p.set_string(kOfxPropType, kOfxTypeParameter);
    p.set_string(kOfxParamPropType, kOfxParamTypeBoolean);
    p.set_string(kOfxPropName, name);
    p.set_string(kOfxPropLabel, label);
    p.set_int(kOfxParamPropDefault, default);
    p.set_int(kOfxParamPropAnimates, 0);
}

// ---------------------------------------------------------------------------
// Images
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Depth {
    Byte,
    Short,
    Float,
}

impl Depth {
    fn sample_size(self) -> usize {
        match self {
            Depth::Byte => 1,
            Depth::Short => 2,
            Depth::Float => 4,
        }
    }
}

struct ImageView {
    data: *mut u8,
    bounds: OfxRectI,
    row_bytes: usize,
    depth: Depth,
    channels: usize,
}

impl ImageView {
    unsafe fn from_handle(img: OfxPropertySetHandle) -> Option<ImageView> {
        let p = Prop(img);
        let data = p.get_pointer(kOfxImagePropData)? as *mut u8;
        let b = p.get_int4(kOfxImagePropBounds)?;
        let row_bytes = p.get_int(kOfxImagePropRowBytes)? as usize;
        let depth = match p.get_string(kOfxImageEffectPropPixelDepth)?.as_str() {
            kOfxBitDepthByte => Depth::Byte,
            kOfxBitDepthShort => Depth::Short,
            kOfxBitDepthFloat => Depth::Float,
            _ => return None,
        };
        let channels = match p.get_string(kOfxImageEffectPropComponents)?.as_str() {
            kOfxImageComponentRGBA => 4,
            kOfxImageComponentRGB => 3,
            kOfxImageComponentAlpha => 1,
            _ => return None,
        };
        Some(ImageView {
            data,
            bounds: OfxRectI {
                x1: b[0],
                y1: b[1],
                x2: b[2],
                y2: b[3],
            },
            row_bytes,
            depth,
            channels,
        })
    }

    fn width(&self) -> usize {
        (self.bounds.x2 - self.bounds.x1).max(0) as usize
    }

    fn height(&self) -> usize {
        (self.bounds.y2 - self.bounds.y1).max(0) as usize
    }

    #[inline]
    unsafe fn read_sample(&self, p: *const u8) -> f32 {
        match self.depth {
            Depth::Byte => *p as f32 / 255.0,
            Depth::Short => (ptr::read_unaligned(p as *const u16)) as f32 / 65535.0,
            Depth::Float => ptr::read_unaligned(p as *const f32),
        }
    }

    #[inline]
    unsafe fn write_sample(&self, p: *mut u8, v: f32) {
        match self.depth {
            Depth::Byte => *p = (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            Depth::Short => {
                ptr::write_unaligned(p as *mut u16, (v.clamp(0.0, 1.0) * 65535.0 + 0.5) as u16)
            }
            Depth::Float => ptr::write_unaligned(p as *mut f32, v),
        }
    }

    unsafe fn read_frame(&self) -> Frame {
        let (w, h) = (self.width(), self.height());
        let mut f = Frame::new(w, h, 4);
        let stride = self.channels * self.depth.sample_size();
        for y in 0..h {
            let row = self.data.add(y * self.row_bytes) as *const u8;
            for x in 0..w {
                let base = row.add(x * stride);
                let r = self.read_sample(base);
                let g = if self.channels > 1 {
                    self.read_sample(base.add(self.depth.sample_size()))
                } else {
                    r
                };
                let b = if self.channels > 2 {
                    self.read_sample(base.add(2 * self.depth.sample_size()))
                } else {
                    g
                };
                let a = if self.channels > 3 {
                    self.read_sample(base.add(3 * self.depth.sample_size()))
                } else {
                    1.0
                };
                let i = (y * w + x) * 4;
                f.data[i] = r;
                f.data[i + 1] = g;
                f.data[i + 2] = b;
                f.data[i + 3] = a;
            }
        }
        f
    }

    unsafe fn write_frame(&self, f: &Frame) {
        let (w, h) = (self.width().min(f.width), self.height().min(f.height));
        let stride = self.channels * self.depth.sample_size();
        let ss = self.depth.sample_size();
        for y in 0..h {
            let row = self.data.add(y * self.row_bytes) as *mut u8;
            for x in 0..w {
                let i = (y * f.width + x) * f.channels;
                let out = row.add(x * stride);
                let r = f.data[i];
                let g = f.data[i + 1];
                let b = f.data[i + 2];
                let a = f.data[i + 3];
                match self.channels {
                    1 => self.write_sample(out, r),
                    3 => {
                        self.write_sample(out, r);
                        self.write_sample(out.add(ss), g);
                        self.write_sample(out.add(2 * ss), b);
                    }
                    _ => {
                        self.write_sample(out, r);
                        self.write_sample(out.add(ss), g);
                        self.write_sample(out.add(2 * ss), b);
                        self.write_sample(out.add(3 * ss), a);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parameters
// ---------------------------------------------------------------------------

struct Values {
    shutter: f32,
    samples: u32,
    amount: f32,
    mix: f32,
    quality: i32,
    use_gpu: bool,
    fg_amount: f32,
    sensitivity: f32,
    display: i32,
    matte_channel: i32,
    matte_shrink: f32,
    max_disp: f32,
    vec_scale_x: f32,
    vec_scale_y: f32,
}

unsafe fn double_value(set: OfxParamSetHandle, name: &str, time: OfxTime) -> Option<f64> {
    let s = suites()?;
    let handle_fn = s.param.paramGetHandle?;
    let get = s.param.paramGetValueAtTime?;
    let mut h: OfxParamHandle = ptr::null_mut();
    let key = cs(name);
    if handle_fn(set, key.as_ptr(), &mut h, ptr::null_mut()) != kOfxStatOK {
        return None;
    }
    let mut v = 0.0f64;
    if get(h, time, &mut v as *mut f64) != kOfxStatOK {
        return None;
    }
    Some(v)
}

unsafe fn int_value(set: OfxParamSetHandle, name: &str, time: OfxTime) -> Option<c_int> {
    let s = suites()?;
    let handle_fn = s.param.paramGetHandle?;
    let get = s.param.paramGetValueAtTime?;
    let mut h: OfxParamHandle = ptr::null_mut();
    let key = cs(name);
    if handle_fn(set, key.as_ptr(), &mut h, ptr::null_mut()) != kOfxStatOK {
        return None;
    }
    let mut v: c_int = 0;
    if get(h, time, &mut v as *mut c_int) != kOfxStatOK {
        return None;
    }
    Some(v)
}

unsafe fn read_values(instance: OfxImageEffectHandle, time: OfxTime) -> Option<Values> {
    let s = suites()?;
    let mut set: OfxParamSetHandle = ptr::null_mut();
    (s.effect.getParamSet?)(instance, &mut set);
    if set.is_null() {
        return None;
    }
    let d = |name: &str, fallback: f64| -> f64 {
        double_value(set, name, time).unwrap_or(fallback)
    };
    let i = |name: &str, fallback: c_int| -> c_int {
        int_value(set, name, time).unwrap_or(fallback)
    };
    Some(Values {
        shutter: d(P_SHUTTER, 0.5) as f32,
        samples: i(P_SAMPLES, 16).max(1) as u32,
        amount: d(P_AMOUNT, 1.0) as f32,
        mix: d(P_MIX, 1.0) as f32,
        quality: i(P_QUALITY, 2).clamp(1, 4),
        use_gpu: i(P_USEGPU, 1) != 0,
        fg_amount: d(P_FGAMOUNT, 1.0) as f32,
        sensitivity: d(P_SENSITIVITY, 70.0) as f32,
        display: i(P_DISPLAY, 0).clamp(0, 2),
        matte_channel: i(P_MATTECH, 0).clamp(0, 4),
        matte_shrink: d(P_MATTESHRINK, 0.0) as f32,
        max_disp: d(P_MAXDISP, 64.0) as f32,
        vec_scale_x: d(P_VECSCALEX, 1.0) as f32,
        vec_scale_y: d(P_VECSCALEY, 1.0) as f32,
    })
}

fn flow_options(quality: i32, max_disp: f32) -> FlowOptions {
    // The pyramid does the heavy lifting: `max_disp` is turned into the search
    // radius of the coarsest level, and each level doubles the reach.
    let levels: usize = match quality {
        1 => 4,
        _ => 5,
    };
    let reach = (1u32 << (levels - 1)) as f32;
    let search = ((max_disp / reach).round() as i32).clamp(1, 32);
    FlowOptions {
        levels,
        search_radius: search,
        block_radius: 1,
        median_passes: 2,
        refine_passes: 2,
        smooth_passes: 2,
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

/// Declare the frames we need from Source, so the host allows temporal access.
pub unsafe fn frames_needed(
    _instance: OfxImageEffectHandle,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxStatus {
    let time = Prop(in_args).get_double(kOfxPropTime).unwrap_or(0.0);
    let key = format!("{kOfxImageClipPropFrameRange}{kOfxImageEffectSimpleSourceClipName}");
    let p = Prop(out_args);
    p.set_double_at(&key, 0, time - 1.0);
    p.set_double_at(&key, 1, time + 1.0);
    crate::host::log(&format!("frames_needed t={time:.3} -> {:.1}..{:.1}", time - 1.0, time + 1.0));
    kOfxStatOK
}

pub unsafe fn is_identity(
    instance: OfxImageEffectHandle,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxStatus {
    let time = Prop(in_args).get_double(kOfxPropTime).unwrap_or(0.0);
    let Some(v) = read_values(instance, time) else {
        return kOfxStatReplyDefault;
    };
    if v.amount > 0.0 && v.shutter > 0.0 {
        return kOfxStatReplyDefault;
    }
    Prop(out_args).set_string(kOfxPropName, kOfxImageEffectSimpleSourceClipName);
    kOfxStatOK
}

pub unsafe fn render(
    instance: OfxImageEffectHandle,
    in_args: OfxPropertySetHandle,
    _out_args: OfxPropertySetHandle,
) -> OfxStatus {
    let Some(s) = suites() else {
        return kOfxStatErrMissingHostFeature;
    };
    let (Some(clip_get), Some(clip_get_image), Some(clip_release)) = (
        s.effect.clipGetHandle,
        s.effect.clipGetImage,
        s.effect.clipReleaseImage,
    ) else {
        return kOfxStatErrMissingHostFeature;
    };
    let time = Prop(in_args).get_double(kOfxPropTime).unwrap_or(0.0);

    let mut src_clip: OfxImageClipHandle = ptr::null_mut();
    if clip_get(instance, cstr!("Source"), &mut src_clip, ptr::null_mut()) != kOfxStatOK {
        return kOfxStatFailed;
    }
    let mut out_clip: OfxImageClipHandle = ptr::null_mut();
    if clip_get(instance, cstr!("Output"), &mut out_clip, ptr::null_mut()) != kOfxStatOK {
        return kOfxStatFailed;
    }

    // Optional inputs. A missing or disconnected clip is a null handle.
    let mut matte_clip: OfxImageClipHandle = ptr::null_mut();
    if !clip_connected(instance, CLIP_MATTE, &mut matte_clip) {
        matte_clip = ptr::null_mut();
    }
    let mut vectors_clip: OfxImageClipHandle = ptr::null_mut();
    if !clip_connected(instance, CLIP_VECTORS, &mut vectors_clip) {
        vectors_clip = ptr::null_mut();
    }
    crate::host::log(&format!(
        "clips: matte_connected={} vectors_connected={}",
        !matte_clip.is_null(),
        !vectors_clip.is_null()
    ));

    let mut out_img: OfxPropertySetHandle = ptr::null_mut();
    if clip_get_image(out_clip, time, ptr::null(), &mut out_img) != kOfxStatOK {
        return kOfxStatFailed;
    }

    let status = render_into(
        instance,
        src_clip,
        matte_clip,
        vectors_clip,
        out_img,
        time,
    );
    clip_release(out_img);
    status
}

/// True when the named optional input has something plugged into it.
unsafe fn clip_connected(
    instance: OfxImageEffectHandle,
    name: &str,
    out: &mut OfxImageClipHandle,
) -> bool {
    let Some(s) = suites() else { return false };
    let Some(clip_get) = s.effect.clipGetHandle else {
        return false;
    };
    let mut props: OfxPropertySetHandle = ptr::null_mut();
    if clip_get(instance, cs(name).as_ptr(), out, &mut props) != kOfxStatOK {
        return false;
    }
    Prop(props)
        .get_int(kOfxImageClipPropConnected)
        .map(|v| v != 0)
        .unwrap_or(false)
}

unsafe fn fetch(clip: OfxImageClipHandle, time: OfxTime) -> Option<OfxPropertySetHandle> {
    if clip.is_null() {
        return None;
    }
    let s = suites()?;
    let mut img: OfxPropertySetHandle = ptr::null_mut();
    let f = s.effect.clipGetImage?;
    if f(clip, time, ptr::null(), &mut img) != kOfxStatOK {
        return None;
    }
    let view = ImageView::from_handle(img)?;
    if view.width() == 0 || view.height() == 0 {
        (s.effect.clipReleaseImage?)(img);
        return None;
    }
    Some(img)
}

unsafe fn load_frame(clip: OfxImageClipHandle, time: OfxTime) -> Option<Frame> {
    let img = fetch(clip, time)?;
    let view = ImageView::from_handle(img)?;
    let frame = view.read_frame();
    if let Some(release) = suites().and_then(|s| s.effect.clipReleaseImage) {
        release(img);
    }
    Some(frame)
}

/// Matte mask in `0..1`, resampled onto the output grid.
///
/// `channel`: 0 luma, 1 red, 2 green, 3 blue, 4 alpha. `shrink` erodes the
/// mask so foreground motion does not bleed across the edge.
unsafe fn load_matte(
    clip: OfxImageClipHandle,
    time: OfxTime,
    w: usize,
    h: usize,
    channel: i32,
    shrink: f32,
) -> Option<Vec<f32>> {
    let f = load_frame(clip, time)?;
    if f.width == 0 || f.height == 0 {
        return None;
    }
    let mut mask = vec![0.0f32; w * h];
    let sx = f.width as f32 / w as f32;
    let sy = f.height as f32 / h as f32;
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 * sx;
            let py = y as f32 * sy;
            let r = f.sample(px, py, 0);
            let g = if f.channels > 1 { f.sample(px, py, 1) } else { r };
            let b = if f.channels > 2 { f.sample(px, py, 2) } else { g };
            let a = if f.channels > 3 { f.sample(px, py, 3) } else { 1.0 };
            let v = match channel {
                1 => r,
                2 => g,
                3 => b,
                4 => a,
                _ => 0.2126 * r + 0.7152 * g + 0.0722 * b,
            };
            mask[y * w + x] = v.clamp(0.0, 1.0);
        }
    }

    let radius = shrink.max(0.0).round() as usize;
    if radius > 0 {
        erode(&mut mask, w, h, radius);
    }
    Some(mask)
}

/// Box-min (erosion) so the matte edge contracts by `radius` pixels.
fn erode(mask: &mut [f32], w: usize, h: usize, radius: usize) {
    let src = mask.to_vec();
    let r = radius as i32;
    for y in 0..h {
        for x in 0..w {
            let mut m = 1.0f32;
            for dy in -r..=r {
                let sy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                for dx in -r..=r {
                    let sx = (x as i32 + dx).clamp(0, w as i32 - 1) as usize;
                    m = m.min(src[sy * w + sx]);
                }
            }
            mask[y * w + x] = m;
        }
    }
}

/// Colour picture of the motion field, for the Display parameter.
fn flow_view(f: &FlowField) -> Frame {
    let mut out = Frame::new(f.width, f.height, 4);
    for y in 0..f.height {
        for x in 0..f.width {
            let (u, v) = f.get(x, y);
            let i = (y * f.width + x) * 4;
            out.data[i] = (0.5 + u / 64.0).clamp(0.0, 1.0);
            out.data[i + 1] = (0.5 + v / 64.0).clamp(0.0, 1.0);
            out.data[i + 2] = 0.5;
            out.data[i + 3] = 1.0;
        }
    }
    out
}

/// External motion vectors: red and green carry the displacement in pixels.
unsafe fn load_vectors(
    clip: OfxImageClipHandle,
    time: OfxTime,
    w: usize,
    h: usize,
    scale_x: f32,
    scale_y: f32,
) -> Option<FlowField> {
    let f = load_frame(clip, time)?;
    if f.width == 0 || f.height == 0 {
        return None;
    }
    let mut flow = FlowField::new(w, h);
    let sx = f.width as f32 / w as f32;
    let sy = f.height as f32 / h as f32;
    for y in 0..h {
        for x in 0..w {
            let px = x as f32 * sx;
            let py = y as f32 * sy;
            let u = f.sample(px, py, 0) * scale_x;
            let v = if f.channels > 1 {
                f.sample(px, py, 1) * scale_y
            } else {
                0.0
            };
            flow.set(x, y, u, v);
        }
    }
    Some(flow)
}

unsafe fn render_into(
    instance: OfxImageEffectHandle,
    src_clip: OfxImageClipHandle,
    matte_clip: OfxImageClipHandle,
    vectors_clip: OfxImageClipHandle,
    out_img: OfxPropertySetHandle,
    time: OfxTime,
) -> OfxStatus {
    let Some(s) = suites() else {
        return kOfxStatErrMissingHostFeature;
    };
    let Some(out_view) = ImageView::from_handle(out_img) else {
        return kOfxStatErrFormat;
    };
    let (w, h) = (out_view.width(), out_view.height());
    if w == 0 || h == 0 {
        return kOfxStatOK;
    }

    let Some(values) = read_values(instance, time) else {
        return kOfxStatFailed;
    };

    // Neighbouring frames. Half a frame is what the shutter needs, but hosts
    // that round clip access to whole frames return the same image twice, so we
    // step a full frame each way and treat the span as the normalised interval.
    let prev_handle = fetch(src_clip, time - 1.0);
    let next_handle = fetch(src_clip, time + 1.0);

    let prev = prev_handle
        .map(|h| ImageView::from_handle(h).map(|v| v.read_frame()))
        .flatten();
    let next = next_handle
        .map(|h| ImageView::from_handle(h).map(|v| v.read_frame()))
        .flatten();

    let result = (|| -> Option<Frame> {
        let prev_f = fit(prev.clone(), w, h);
        let next_f = fit(next.clone(), w, h);
        let matte = load_matte(
            matte_clip,
            time,
            w,
            h,
            values.matte_channel,
            values.matte_shrink,
        );
        let params = Params {
            t: 0.5,
            shutter: values.shutter,
            samples: values.samples,
            amount: values.amount,
            fg_amount: values.fg_amount,
            mix: values.mix,
            // Sensitivity 100 means "trust the estimate", 0 means "only use
            // samples the forward/backward check agrees with".
            occlusion: (1.0 - values.sensitivity / 100.0).clamp(0.0, 1.0),
            color_sigma: 0.15,
            flow: flow_options(values.quality, values.max_disp),
        };

        // External vectors replace the estimate entirely when the clip is wired.
        let external = load_vectors(
            vectors_clip,
            time,
            w,
            h,
            values.vec_scale_x,
            values.vec_scale_y,
        );

        // Some hosts report an optional clip as connected and hand back a black
        // image. A field that is zero everywhere while the frames differ cannot
        // be a real vector pass, so fall back to estimation.
        let external = external.filter(|v| {
            let max = v
                .data
                .iter()
                .fold(0.0f32, |m, x| m.max(x.abs()));
            if max > 1e-4 {
                return true;
            }
            let frames_differ = prev_f
                .data
                .iter()
                .zip(next_f.data.iter())
                .any(|(a, b)| (a - b).abs() > 1.0 / 512.0);
            !frames_differ
        });
        let (forward, backward) = match &external {
            Some(v) => {
                // Linear motion: the backward field is the negated forward one.
                let mut back = v.clone();
                for x in back.data.iter_mut() {
                    *x = -*x;
                }
                (v.clone(), back)
            }
            None => (
                dense_flow(&prev_f, &next_f, &params.flow),
                dense_flow(&next_f, &prev_f, &params.flow),
            ),
        };

        crate::host::log(&format!(
            "render {w}x{h} t={time:.3} amount={:.2} shutter={:.2} samples={} motion={:.3}px gpu={} matte={} vectors={}",
            params.amount,
            params.shutter,
            params.samples,
            forward.mean_magnitude(),
            values.use_gpu,
            matte.is_some(),
            external.is_some()
        ));

        // Display 1 shows the field itself, 2 shows the unblurred frame.
        if values.display == 1 {
            return Some(flow_view(&forward));
        }
        if values.display == 2 {
            let unblurred = Params {
                shutter: 0.0,
                ..params.clone()
            };
            return Some(wsmb_core::render_with(
                &prev_f,
                &next_f,
                &forward,
                &backward,
                &unblurred,
                matte.as_deref(),
            ));
        }

        let blurred = if values.use_gpu {
            wsmb_gpu::global().and_then(|gpu| {
                gpu.render(&prev_f, &next_f, &forward, &backward, &params, matte.as_deref())
            })
        } else {
            None
        };
        Some(match blurred {
            Some(f) => f,
            None => wsmb_core::render_with(
                &prev_f,
                &next_f,
                &forward,
                &backward,
                &params,
                matte.as_deref(),
            ),
        })
    })();

    if let Some(release) = s.effect.clipReleaseImage {
        if let Some(h) = prev_handle {
            release(h);
        }
        if let Some(h) = next_handle {
            release(h);
        }
    }

    let Some(frame) = result else {
        return kOfxStatFailed;
    };
    out_view.write_frame(&frame);
    kOfxStatOK
}

/// Pad or crop a frame to the output size so the pipeline always agrees.
fn fit(src: Option<Frame>, w: usize, h: usize) -> Frame {
    match src {
        Some(f) if f.width == w && f.height == h => f,
        Some(f) => {
            let mut out = Frame::new(w, h, 4);
            let (cw, ch) = (f.width.min(w), f.height.min(h));
            for y in 0..ch {
                for x in 0..cw {
                    for c in 0..4 {
                        let v = f.data[(y * f.width + x) * f.channels + c.min(f.channels - 1)];
                        out.data[(y * w + x) * 4 + c] = v;
                    }
                }
            }
            out
        }
        None => {
            let mut f = Frame::new(w, h, 4);
            f.data.fill(0.0);
            f
        }
    }
}

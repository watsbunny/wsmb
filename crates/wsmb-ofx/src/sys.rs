//! Hand written bindings for the parts of the OFX 1.4 API that WSMB uses.
//!
//! The layouts and signatures are transcribed from the vendored OpenFX headers
//! in `vendor/openfx/include` (BSD-3-Clause, Academy Software Foundation).
//! Keeping them hand written means the plugin builds with no C compiler and no
//! libclang at all.

#![allow(non_snake_case, non_camel_case_types, dead_code)]

use core::ffi::{c_char, c_int, c_uint, c_void};

pub type OfxStatus = c_int;
pub type OfxPropertySetHandle = *mut c_void;
pub type OfxImageEffectHandle = *mut c_void;
pub type OfxImageClipHandle = *mut c_void;
pub type OfxParamSetHandle = *mut c_void;
pub type OfxParamHandle = *mut c_void;
pub type OfxTime = f64;

pub const kOfxStatOK: OfxStatus = 0;
pub const kOfxStatFailed: OfxStatus = 1;
pub const kOfxStatErrFatal: OfxStatus = 2;
pub const kOfxStatErrUnknown: OfxStatus = 3;
pub const kOfxStatErrMissingHostFeature: OfxStatus = 4;
pub const kOfxStatErrUnsupported: OfxStatus = 5;
pub const kOfxStatErrExists: OfxStatus = 6;
pub const kOfxStatErrFormat: OfxStatus = 7;
pub const kOfxStatErrMemory: OfxStatus = 8;
pub const kOfxStatErrBadHandle: OfxStatus = 9;
pub const kOfxStatErrBadIndex: OfxStatus = 10;
pub const kOfxStatErrValue: OfxStatus = 11;
pub const kOfxStatReplyDefault: OfxStatus = 14;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OfxRectI {
    pub x1: c_int,
    pub y1: c_int,
    pub x2: c_int,
    pub y2: c_int,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct OfxRectD {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
}

pub type OfxPluginEntryPoint = unsafe extern "C" fn(
    action: *const c_char,
    handle: *const c_void,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxStatus;

pub type OfxFetchSuite =
    unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int) -> *const c_void;

#[repr(C)]
pub struct OfxHost {
    pub host: OfxPropertySetHandle,
    pub fetch_suite: Option<OfxFetchSuite>,
}

#[repr(C)]
pub struct OfxPlugin {
    pub pluginApi: *const c_char,
    pub apiVersion: c_int,
    pub pluginIdentifier: *const c_char,
    pub pluginVersionMajor: c_uint,
    pub pluginVersionMinor: c_uint,
    pub setHost: Option<unsafe extern "C" fn(*mut OfxHost)>,
    pub mainEntry: Option<OfxPluginEntryPoint>,
}

// ---------------------------------------------------------------------------
// ofxProperty.h
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct OfxPropertySuiteV1 {
    pub propSetPointer:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut c_void) -> OfxStatus>,
    pub propSetString:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *const c_char) -> OfxStatus>,
    pub propSetDouble:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, f64) -> OfxStatus>,
    pub propSetInt:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, c_int) -> OfxStatus>,
    pub propSetPointerN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *const *mut c_void) -> OfxStatus>,
    pub propSetStringN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *const *const c_char) -> OfxStatus>,
    pub propSetDoubleN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *const f64) -> OfxStatus>,
    pub propSetIntN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *const c_int) -> OfxStatus>,
    pub propGetPointer:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut *mut c_void) -> OfxStatus>,
    pub propGetString:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut *mut c_char) -> OfxStatus>,
    pub propGetDouble:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut f64) -> OfxStatus>,
    pub propGetInt:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut c_int) -> OfxStatus>,
    pub propGetPointerN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut *mut c_void) -> OfxStatus>,
    pub propGetStringN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut *mut c_char) -> OfxStatus>,
    pub propGetDoubleN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut f64) -> OfxStatus>,
    pub propGetIntN:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, c_int, *mut c_int) -> OfxStatus>,
    pub propReset: Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char) -> OfxStatus>,
    pub propGetDimension:
        Option<unsafe extern "C" fn(OfxPropertySetHandle, *const c_char, *mut c_int) -> OfxStatus>,
}

// ---------------------------------------------------------------------------
// ofxImageEffect.h
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct OfxImageEffectSuiteV1 {
    pub getPropertySet:
        Option<unsafe extern "C" fn(OfxImageEffectHandle, *mut OfxPropertySetHandle) -> OfxStatus>,
    pub getParamSet:
        Option<unsafe extern "C" fn(OfxImageEffectHandle, *mut OfxParamSetHandle) -> OfxStatus>,
    pub clipDefine:
        Option<unsafe extern "C" fn(OfxImageEffectHandle, *const c_char, *mut OfxPropertySetHandle) -> OfxStatus>,
    pub clipGetHandle: Option<
        unsafe extern "C" fn(
            OfxImageEffectHandle,
            *const c_char,
            *mut OfxImageClipHandle,
            *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub clipGetPropertySet:
        Option<unsafe extern "C" fn(OfxImageClipHandle, *mut OfxPropertySetHandle) -> OfxStatus>,
    pub clipGetImage: Option<
        unsafe extern "C" fn(
            OfxImageClipHandle,
            OfxTime,
            *const OfxRectD,
            *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub clipReleaseImage: Option<unsafe extern "C" fn(OfxPropertySetHandle) -> OfxStatus>,
    pub clipGetRegionOfDefinition:
        Option<unsafe extern "C" fn(OfxImageClipHandle, OfxTime, *mut OfxRectD) -> OfxStatus>,
    pub abort: Option<unsafe extern "C" fn(OfxImageEffectHandle) -> c_int>,
    pub imageMemoryAlloc: Option<
        unsafe extern "C" fn(OfxImageEffectHandle, usize, *mut *mut c_void) -> OfxStatus,
    >,
    pub imageMemoryFree: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
    pub imageMemoryLock:
        Option<unsafe extern "C" fn(*mut c_void, *mut *mut c_void) -> OfxStatus>,
    pub imageMemoryUnlock: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
}

// ---------------------------------------------------------------------------
// ofxParam.h
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct OfxParameterSuiteV1 {
    pub paramDefine: Option<
        unsafe extern "C" fn(
            OfxParamSetHandle,
            *const c_char,
            *const c_char,
            *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub paramGetHandle: Option<
        unsafe extern "C" fn(
            OfxParamSetHandle,
            *const c_char,
            *mut OfxParamHandle,
            *mut OfxPropertySetHandle,
        ) -> OfxStatus,
    >,
    pub paramSetGetPropertySet:
        Option<unsafe extern "C" fn(OfxParamSetHandle, *mut OfxPropertySetHandle) -> OfxStatus>,
    pub paramGetPropertySet:
        Option<unsafe extern "C" fn(OfxParamHandle, *mut OfxPropertySetHandle) -> OfxStatus>,
    pub paramGetValue: Option<unsafe extern "C" fn(OfxParamHandle, ...) -> OfxStatus>,
    pub paramGetValueAtTime:
        Option<unsafe extern "C" fn(OfxParamHandle, OfxTime, ...) -> OfxStatus>,
    pub paramGetDerivative:
        Option<unsafe extern "C" fn(OfxParamHandle, OfxTime, ...) -> OfxStatus>,
    pub paramGetIntegral:
        Option<unsafe extern "C" fn(OfxParamHandle, OfxTime, OfxTime, ...) -> OfxStatus>,
    pub paramSetValue: Option<unsafe extern "C" fn(OfxParamHandle, ...) -> OfxStatus>,
    pub paramSetValueAtTime:
        Option<unsafe extern "C" fn(OfxParamHandle, OfxTime, ...) -> OfxStatus>,
    pub paramGetNumKeys:
        Option<unsafe extern "C" fn(OfxParamHandle, *mut c_uint) -> OfxStatus>,
    pub paramGetKeyTime:
        Option<unsafe extern "C" fn(OfxParamHandle, c_uint, *mut OfxTime) -> OfxStatus>,
    pub paramGetKeyIndex:
        Option<unsafe extern "C" fn(OfxParamHandle, OfxTime, c_int, *mut c_int) -> OfxStatus>,
    pub paramDeleteKey: Option<unsafe extern "C" fn(OfxParamHandle, OfxTime) -> OfxStatus>,
    pub paramDeleteAllKeys: Option<unsafe extern "C" fn(OfxParamHandle) -> OfxStatus>,
    pub paramCopy: Option<
        unsafe extern "C" fn(OfxParamHandle, OfxParamHandle, OfxTime, *const [f64; 2]) -> OfxStatus,
    >,
    pub paramEditBegin:
        Option<unsafe extern "C" fn(OfxParamSetHandle, *const c_char) -> OfxStatus>,
    pub paramEditEnd: Option<unsafe extern "C" fn(OfxParamSetHandle) -> OfxStatus>,
}

// ---------------------------------------------------------------------------
// ofxMemory.h / ofxMultiThread.h — declared for completeness, unused so far
// ---------------------------------------------------------------------------

#[repr(C)]
pub struct OfxMemorySuiteV1 {
    pub memoryAlloc: Option<unsafe extern "C" fn(usize, *mut *mut c_void) -> OfxStatus>,
    pub memoryFree: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
}

#[repr(C)]
pub struct OfxMultiThreadSuiteV1 {
    pub multiThread:
        Option<unsafe extern "C" fn(unsafe extern "C" fn(c_uint, c_uint, *mut c_void), c_uint, *mut c_void) -> OfxStatus>,
    pub multiThreadNumCPUs: Option<unsafe extern "C" fn(*mut c_int) -> OfxStatus>,
    pub multiThreadIndex: Option<unsafe extern "C" fn(*mut c_int) -> OfxStatus>,
    pub multiThreadIsSpawnedThread: Option<unsafe extern "C" fn() -> c_int>,
    pub mutexCreate:
        Option<unsafe extern "C" fn(*mut *mut c_void, c_int) -> OfxStatus>,
    pub mutexDestroy: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
    pub mutexLock: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
    pub mutexUnlock: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
    pub mutexTryLock: Option<unsafe extern "C" fn(*mut c_void) -> OfxStatus>,
}

// ---------------------------------------------------------------------------
// String constants
// ---------------------------------------------------------------------------

pub const kOfxImageEffectPluginApi: &str = "OfxImageEffectPluginAPI";

pub const kOfxActionLoad: &str = "OfxActionLoad";
pub const kOfxActionUnload: &str = "OfxActionUnload";
pub const kOfxActionDescribe: &str = "OfxActionDescribe";
pub const kOfxActionPurgeCaches: &str = "OfxActionPurgeCaches";
pub const kOfxActionSyncPrivateData: &str = "OfxActionSyncPrivateData";
pub const kOfxActionCreateInstance: &str = "OfxActionCreateInstance";
pub const kOfxActionDestroyInstance: &str = "OfxActionDestroyInstance";
pub const kOfxActionInstanceChanged: &str = "OfxActionInstanceChanged";
pub const kOfxActionBeginInstanceChanged: &str = "OfxActionBeginInstanceChanged";
pub const kOfxActionEndInstanceChanged: &str = "OfxActionEndInstanceChanged";

pub const kOfxImageEffectActionDescribeInContext: &str = "OfxImageEffectActionDescribeInContext";
pub const kOfxImageEffectActionRender: &str = "OfxImageEffectActionRender";
pub const kOfxImageEffectActionIsIdentity: &str = "OfxImageEffectActionIsIdentity";
pub const kOfxImageEffectActionGetRegionOfDefinition: &str =
    "OfxImageEffectActionGetRegionOfDefinition";
pub const kOfxImageEffectActionGetClipPreferences: &str =
    "OfxImageEffectActionGetClipPreferences";
pub const kOfxImageEffectActionGetFramesNeeded: &str =
    "OfxImageEffectActionGetFramesNeeded";

pub const kOfxImageClipPropFrameRange: &str = "OfxImageClipPropFrameRange_";
pub const kOfxImageClipPropContinuousSamples: &str = "OfxImageClipPropContinuousSamples";

pub const kOfxPropName: &str = "OfxPropName";
pub const kOfxPropLabel: &str = "OfxPropLabel";
pub const kOfxPropShortLabel: &str = "OfxPropShortLabel";
pub const kOfxPropLongLabel: &str = "OfxPropLongLabel";
pub const kOfxPropType: &str = "OfxPropType";
pub const kOfxPropTime: &str = "OfxPropTime";
pub const kOfxPropVersion: &str = "OfxPropVersion";
pub const kOfxPropVersionLabel: &str = "OfxPropVersionLabel";

pub const kOfxTypeImageEffect: &str = "OfxTypeImageEffect";
pub const kOfxTypeParameter: &str = "OfxTypeParameter";
pub const kOfxTypeClip: &str = "OfxTypeClip";

pub const kOfxImageEffectPropContext: &str = "OfxImageEffectPropContext";
pub const kOfxImageEffectContextFilter: &str = "OfxImageEffectContextFilter";
pub const kOfxImageEffectContextGeneral: &str = "OfxImageEffectContextGeneral";
pub const kOfxImageEffectPropSupportedContexts: &str = "OfxImageEffectPropSupportedContexts";
pub const kOfxImageEffectPropSupportedPixelDepths: &str =
    "OfxImageEffectPropSupportedPixelDepths";
pub const kOfxImageEffectPropSupportedComponents: &str = "OfxImageEffectPropSupportedComponents";
pub const kOfxImageEffectPropTemporalClipAccess: &str = "OfxImageEffectPropTemporalClipAccess";
pub const kOfxImageEffectPropSupportsTiles: &str = "OfxImageEffectPropSupportsTiles";
pub const kOfxImageEffectPropPixelDepth: &str = "OfxImageEffectPropPixelDepth";
pub const kOfxImageEffectPropComponents: &str = "OfxImageEffectPropComponents";
pub const kOfxImageEffectPropPreMultiplication: &str = "OfxImageEffectPropPreMultiplication";
pub const kOfxImageEffectPropRenderScale: &str = "OfxImageEffectPropRenderScale";
pub const kOfxImageEffectPropRenderWindow: &str = "OfxImageEffectPropRenderWindow";
pub const kOfxImageEffectPropRegionOfInterest: &str = "OfxImageEffectPropRegionOfInterest";
pub const kOfxImageEffectPropFrameRate: &str = "OfxImageEffectPropFrameRate";
pub const kOfxImageEffectPropSequentialRenderStatus: &str =
    "OfxImageEffectPropSequentialRenderStatus";
pub const kOfxImageEffectPropInteractiveRenderStatus: &str =
    "OfxImageEffectPropInteractiveRenderStatus";
pub const kOfxImageEffectPluginPropGrouping: &str = "OfxImageEffectPluginPropGrouping";

pub const kOfxImageEffectSimpleSourceClipName: &str = "Source";
pub const kOfxImageEffectOutputClipName: &str = "Output";

pub const kOfxImageClipPropOptional: &str = "OfxImageClipPropOptional";
pub const kOfxImagePropData: &str = "OfxImagePropData";
pub const kOfxImagePropBounds: &str = "OfxImagePropBounds";
pub const kOfxImagePropRowBytes: &str = "OfxImagePropRowBytes";

pub const kOfxImageComponentRGBA: &str = "OfxImageComponentRGBA";
pub const kOfxImageComponentRGB: &str = "OfxImageComponentRGB";
pub const kOfxImageComponentAlpha: &str = "OfxImageComponentAlpha";

pub const kOfxBitDepthByte: &str = "OfxBitDepthByte";
pub const kOfxBitDepthShort: &str = "OfxBitDepthShort";
pub const kOfxBitDepthFloat: &str = "OfxBitDepthFloat";

pub const kOfxParamTypeDouble: &str = "OfxParamTypeDouble";
pub const kOfxParamTypeInteger: &str = "OfxParamTypeInteger";
pub const kOfxParamTypeBoolean: &str = "OfxParamTypeBoolean";
pub const kOfxParamTypeChoice: &str = "OfxParamTypeChoice";
pub const kOfxParamTypeGroup: &str = "OfxParamTypeGroup";
pub const kOfxParamTypePushButton: &str = "OfxParamTypePushButton";

pub const kOfxParamPropType: &str = "OfxParamPropType";
pub const kOfxParamPropDoubleType: &str = "OfxParamPropDoubleType";
pub const kOfxParamPropDefault: &str = "OfxParamPropDefault";
pub const kOfxParamPropMin: &str = "OfxParamPropMin";
pub const kOfxParamPropMax: &str = "OfxParamPropMax";
pub const kOfxParamPropDisplayMin: &str = "OfxParamPropDisplayMin";
pub const kOfxParamPropDisplayMax: &str = "OfxParamPropDisplayMax";
pub const kOfxParamPropAnimates: &str = "OfxParamPropAnimates";
pub const kOfxParamPropIncrement: &str = "OfxParamPropIncrement";
pub const kOfxParamPropParent: &str = "OfxParamPropParent";
pub const kOfxParamDoubleTypeScale: &str = "OfxParamDoubleTypeScale";
pub const kOfxParamDoubleTypePlain: &str = "OfxParamDoubleTypePlain";

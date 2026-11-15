//! WSMB — Watsbunny Smart Motion Blur, OFX entry points.
//!
//! A host loads this shared library and calls `OfxGetNumberOfPlugins` /
//! `OfxGetPlugin`, then drives the plugin through `OfxPlugin::mainEntry`.

// OFX constants are named kOfxSomething by the spec; keep those names.
#![allow(non_upper_case_globals)]

mod host;
mod plugin;
mod sys;

use core::ffi::{c_char, c_int, c_void};
use core::ptr;
use std::ffi::CStr;

use sys::{OfxHost, OfxPlugin, OfxPropertySetHandle, OfxStatus};

// ---------------------------------------------------------------------------
// Plugin table
// ---------------------------------------------------------------------------

unsafe extern "C" fn set_host(host: *mut OfxHost) {
    host::bind(host);
}

unsafe extern "C" fn main_entry(
    action: *const c_char,
    handle: *const c_void,
    in_args: OfxPropertySetHandle,
    out_args: OfxPropertySetHandle,
) -> OfxStatus {
    if action.is_null() {
        return sys::kOfxStatFailed;
    }
    let action = match CStr::from_ptr(action).to_str() {
        Ok(a) => a,
        Err(_) => return sys::kOfxStatFailed,
    };
    host::log(&format!("action {action}"));

    if host::suites().is_none() {
        return sys::kOfxStatErrMissingHostFeature;
    }

    // Actions every plugin must tolerate.
    if action == sys::kOfxActionLoad
        || action == sys::kOfxActionUnload
        || action == sys::kOfxActionCreateInstance
        || action == sys::kOfxActionDestroyInstance
        || action == sys::kOfxActionPurgeCaches
        || action == sys::kOfxActionSyncPrivateData
        || action == sys::kOfxActionBeginInstanceChanged
        || action == sys::kOfxActionEndInstanceChanged
        || action == sys::kOfxActionInstanceChanged
    {
        return sys::kOfxStatOK;
    }

    if action == sys::kOfxActionDescribe {
        return plugin::describe(handle as OfxPropertySetHandle);
    }
    if action == sys::kOfxImageEffectActionDescribeInContext {
        return plugin::describe_in_context(handle as sys::OfxImageEffectHandle, in_args);
    }
    if action == sys::kOfxImageEffectActionIsIdentity {
        return plugin::is_identity(handle as sys::OfxImageEffectHandle, in_args, out_args);
    }
    if action == sys::kOfxImageEffectActionGetFramesNeeded {
        return plugin::frames_needed(handle as sys::OfxImageEffectHandle, in_args, out_args);
    }
    if action == sys::kOfxImageEffectActionRender {
        return plugin::render(handle as sys::OfxImageEffectHandle, in_args, out_args);
    }

    sys::kOfxStatReplyDefault
}

static mut PLUGIN: OfxPlugin = OfxPlugin {
    pluginApi: b"OfxImageEffectPluginAPI\0".as_ptr() as *const c_char,
    apiVersion: 1,
    pluginIdentifier: b"io.github.watsbunny.wsmb\0".as_ptr() as *const c_char,
    pluginVersionMajor: 0,
    pluginVersionMinor: 1,
    setHost: Some(set_host),
    mainEntry: Some(main_entry),
};

#[no_mangle]
pub extern "C" fn OfxGetNumberOfPlugins() -> c_int {
    1
}

#[no_mangle]
pub extern "C" fn OfxGetPlugin(nth: c_int) -> *mut OfxPlugin {
    if nth == 0 {
        ptr::addr_of_mut!(PLUGIN)
    } else {
        ptr::null_mut()
    }
}

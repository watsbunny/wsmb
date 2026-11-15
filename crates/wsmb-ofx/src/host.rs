//! Suite bootstrap and thin, unsafe property helpers.

use crate::sys::*;
use core::ffi::{c_char, c_int, c_void};
use std::ffi::{CStr, CString};
use std::sync::OnceLock;

/// Null terminated literal for the C API.
macro_rules! cstr {
    ($s:literal) => {
        concat!($s, "\0").as_ptr() as *const ::core::ffi::c_char
    };
}
pub(crate) use cstr;

pub struct Suites {
    pub prop: &'static OfxPropertySuiteV1,
    pub effect: &'static OfxImageEffectSuiteV1,
    pub param: &'static OfxParameterSuiteV1,
}

// The suite tables are owned by the host and live as long as the module stays
// loaded, which is exactly the lifetime we claim here.
unsafe impl Send for Suites {}
unsafe impl Sync for Suites {}

static SUITES: OnceLock<Suites> = OnceLock::new();

/// Bind to the host's suites. Idempotent.
pub fn bind(host: *mut OfxHost) -> bool {
    if SUITES.get().is_some() {
        return true;
    }
    unsafe {
        let host = match host.as_ref() {
            Some(h) => h,
            None => return false,
        };
        let fetch = match host.fetch_suite {
            Some(f) => f,
            None => return false,
        };

        let prop = fetch(host.host, cstr!("OfxPropertySuite"), 1) as *const OfxPropertySuiteV1;
        let effect =
            fetch(host.host, cstr!("OfxImageEffectSuite"), 1) as *const OfxImageEffectSuiteV1;
        let param = fetch(host.host, cstr!("OfxParameterSuite"), 1) as *const OfxParameterSuiteV1;

        if prop.is_null() || effect.is_null() || param.is_null() {
            return false;
        }

        let suites = Suites {
            prop: &*prop,
            effect: &*effect,
            param: &*param,
        };
        SUITES.set(suites).is_ok()
    }
}

#[inline]
pub fn suites() -> Option<&'static Suites> {
    SUITES.get()
}

/// Diagnostics, off unless `WSMB_LOG` is set in the host's environment.
///
/// The OFX plugin runs inside the host's helper process, so a file is the only
/// way to see what it does while a host drives it.
pub fn log(msg: &str) {
    use std::io::Write;
    use std::sync::OnceLock;

    static ENABLED: OnceLock<bool> = OnceLock::new();
    if !*ENABLED.get_or_init(|| std::env::var_os("WSMB_LOG").is_some()) {
        return;
    }

    let path = std::env::temp_dir().join("wsmb-ofx.log");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{msg}");
    }
}

/// A property set handle with typed accessors.
#[derive(Clone, Copy)]
pub struct Prop(pub OfxPropertySetHandle);

impl Prop {
    /// Host may hand us a null set (describe actions pass null in/out args).
    #[inline]
    pub fn is_null(&self) -> bool {
        self.0.is_null()
    }

    #[inline]
    fn suite(&self) -> Option<&'static OfxPropertySuiteV1> {
        suites().map(|s| s.prop)
    }

    pub unsafe fn set_string(&self, key: &str, value: &str) -> OfxStatus {
        if self.is_null() {
            return kOfxStatErrBadHandle;
        }
        let Some(s) = self.suite() else {
            return kOfxStatErrMissingHostFeature;
        };
        let (Ok(k), Ok(v)) = (CString::new(key), CString::new(value)) else {
            return kOfxStatErrValue;
        };
        match s.propSetString {
            Some(f) => f(self.0, k.as_ptr(), 0, v.as_ptr()),
            None => kOfxStatErrMissingHostFeature,
        }
    }

    pub unsafe fn set_double_at(&self, key: &str, index: c_int, value: f64) -> OfxStatus {
        if self.is_null() {
            return kOfxStatErrBadHandle;
        }
        let Some(s) = self.suite() else {
            return kOfxStatErrMissingHostFeature;
        };
        let Ok(k) = CString::new(key) else {
            return kOfxStatErrValue;
        };
        match s.propSetDouble {
            Some(f) => f(self.0, k.as_ptr(), index, value),
            None => kOfxStatErrMissingHostFeature,
        }
    }

    pub unsafe fn set_double(&self, key: &str, value: f64) -> OfxStatus {
        if self.is_null() {
            return kOfxStatErrBadHandle;
        }
        let Some(s) = self.suite() else {
            return kOfxStatErrMissingHostFeature;
        };
        let Ok(k) = CString::new(key) else {
            return kOfxStatErrValue;
        };
        match s.propSetDouble {
            Some(f) => f(self.0, k.as_ptr(), 0, value),
            None => kOfxStatErrMissingHostFeature,
        }
    }

    pub unsafe fn set_int(&self, key: &str, value: c_int) -> OfxStatus {
        if self.is_null() {
            return kOfxStatErrBadHandle;
        }
        let Some(s) = self.suite() else {
            return kOfxStatErrMissingHostFeature;
        };
        let Ok(k) = CString::new(key) else {
            return kOfxStatErrValue;
        };
        match s.propSetInt {
            Some(f) => f(self.0, k.as_ptr(), 0, value),
            None => kOfxStatErrMissingHostFeature,
        }
    }

    /// Multi-valued string property. The reference OFX plugins set these one
    /// index at a time, which is also what every host reliably implements.
    pub unsafe fn set_strings(&self, key: &str, values: &[&str]) -> OfxStatus {
        let mut status = kOfxStatOK;
        for (i, v) in values.iter().enumerate() {
            let r = self.set_string_at(key, i as c_int, v);
            if r != kOfxStatOK {
                status = r;
            }
        }
        status
    }

    pub unsafe fn set_string_at(&self, key: &str, index: c_int, value: &str) -> OfxStatus {
        if self.is_null() {
            return kOfxStatErrBadHandle;
        }
        let Some(s) = self.suite() else {
            return kOfxStatErrMissingHostFeature;
        };
        let (Ok(k), Ok(v)) = (CString::new(key), CString::new(value)) else {
            return kOfxStatErrValue;
        };
        match s.propSetString {
            Some(f) => f(self.0, k.as_ptr(), index, v.as_ptr()),
            None => kOfxStatErrMissingHostFeature,
        }
    }

    pub unsafe fn get_string(&self, key: &str) -> Option<String> {
        if self.is_null() {
            return None;
        }
        let s = self.suite()?;
        let k = CString::new(key).ok()?;
        let mut raw: *mut c_char = std::ptr::null_mut();
        let f = s.propGetString?;
        if f(self.0, k.as_ptr(), 0, &mut raw) != kOfxStatOK || raw.is_null() {
            return None;
        }
        Some(CStr::from_ptr(raw).to_string_lossy().into_owned())
    }

    pub unsafe fn get_double(&self, key: &str) -> Option<f64> {
        if self.is_null() {
            return None;
        }
        let s = self.suite()?;
        let k = CString::new(key).ok()?;
        let mut v = 0.0f64;
        let f = s.propGetDouble?;
        (f(self.0, k.as_ptr(), 0, &mut v) == kOfxStatOK).then_some(v)
    }

    pub unsafe fn get_int(&self, key: &str) -> Option<c_int> {
        if self.is_null() {
            return None;
        }
        let s = self.suite()?;
        let k = CString::new(key).ok()?;
        let mut v: c_int = 0;
        let f = s.propGetInt?;
        (f(self.0, k.as_ptr(), 0, &mut v) == kOfxStatOK).then_some(v)
    }

    pub unsafe fn get_int4(&self, key: &str) -> Option<[c_int; 4]> {
        if self.is_null() {
            return None;
        }
        let s = self.suite()?;
        let k = CString::new(key).ok()?;
        let mut v = [0 as c_int; 4];
        let f = s.propGetIntN?;
        (f(self.0, k.as_ptr(), 4, v.as_mut_ptr()) == kOfxStatOK).then_some(v)
    }

    pub unsafe fn get_pointer(&self, key: &str) -> Option<*mut c_void> {
        if self.is_null() {
            return None;
        }
        let s = self.suite()?;
        let k = CString::new(key).ok()?;
        let mut raw: *mut c_void = std::ptr::null_mut();
        let f = s.propGetPointer?;
        (f(self.0, k.as_ptr(), 0, &mut raw) == kOfxStatOK).then_some(raw)
    }
}

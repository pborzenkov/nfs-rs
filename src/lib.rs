mod client;
mod error;
mod into_url;

use std::ffi::CStr;

pub use self::client::Client;
pub use self::error::{Error, Result};
pub use self::into_url::IntoUrl;

trait ToStringLossy {
    fn to_string_lossy(&self) -> String;
}

impl ToStringLossy for *mut i8 {
    fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(unsafe { CStr::from_ptr(*self) }.to_bytes()).to_string()
    }
}

impl ToStringLossy for *const i8 {
    fn to_string_lossy(&self) -> String {
        String::from_utf8_lossy(unsafe { CStr::from_ptr(*self) }.to_bytes()).to_string()
    }
}

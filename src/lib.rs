mod buf;
mod client;
mod error;
mod file;
mod into_url;

use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;

pub use self::client::Client;
pub use self::error::{Error, Result};
pub use self::file::File;
pub use self::into_url::IntoUrl;
pub use libnfs_sys::nfs_stat_64 as Stat;

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

trait AsCString {
    fn as_cstring(&self) -> Result<CString>;
}

impl<T> AsCString for T
where
    T: AsRef<std::path::Path>,
{
    fn as_cstring(&self) -> Result<CString> {
        CString::new(self.as_ref().as_os_str().as_bytes())
            .map_err(|e| crate::error::nfs("invalid path", e))
    }
}

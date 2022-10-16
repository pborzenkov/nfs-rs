use crate::{AsCString, ToStringLossy};
use libnfs_sys as libnfs;
use nix::{fcntl::OFlag, sys::stat::Mode, unistd::AccessFlags};
use std::{ffi::CString, io, mem, path::Path, sync::Arc};
use tokio::task;

pub(crate) struct Context(pub(crate) *mut libnfs::nfs_context);

impl Context {
    fn get_last_error(&self) -> String {
        unsafe { libnfs::nfs_get_error(self.0) }.to_string_lossy()
    }

    pub(crate) fn check_retcode_ret(&self, code: i32) -> crate::Result<i32> {
        if code < 0 {
            Err(crate::error::nfs(
                self.get_last_error(),
                io::Error::from_raw_os_error(-code),
            ))
        } else {
            Ok(code)
        }
    }

    pub(crate) fn check_retcode(&self, code: i32) -> crate::Result<()> {
        self.check_retcode_ret(code)?;

        Ok(())
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

struct Url(*const libnfs::nfs_url);

impl Drop for Url {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { libnfs::nfs_destroy_url(self.0 as _) }
        }
    }
}

unsafe impl Send for Url {}

pub struct Client {
    context: Arc<Context>,
}

impl Client {
    pub async fn mount<T: crate::IntoUrl>(url: T) -> crate::Result<Client> {
        let context = unsafe {
            let context = Arc::new(Context(libnfs::nfs_init_context()));
            if context.0.is_null() {
                return Err(crate::error::nfs(
                    "can't initialize libnfs context",
                    io::ErrorKind::OutOfMemory,
                ));
            }

            let url = CString::new(url.into_url()?.as_str())
                .map_err(|e| crate::error::nfs("can't parse URL", e))?;
            let url = Url(libnfs::nfs_parse_url_dir(context.0, url.as_ptr()));
            if url.0.is_null() {
                return Err(crate::error::nfs(
                    context.get_last_error(),
                    io::ErrorKind::InvalidInput,
                ));
            }

            {
                let context = Arc::clone(&context);
                task::spawn_blocking(move || {
                    let url = url;

                    context.check_retcode(libnfs::nfs_mount(
                        context.0,
                        (*url.0).server,
                        (*url.0).path,
                    ))?;
                    context.check_retcode(libnfs::nfs_mt_service_thread_start(context.0))
                })
                .await??;
            }

            context
        };

        Ok(Client { context })
    }

    pub async fn umount(self) -> crate::Result<()> {
        let context = Arc::clone(&self.context);

        task::spawn_blocking(move || unsafe {
            libnfs::nfs_mt_service_thread_stop(context.0);
            context.check_retcode(libnfs::nfs_umount(context.0))
        })
        .await?
    }

    pub async fn access<P: AsRef<Path>>(&self, path: P) -> crate::Result<AccessFlags> {
        let context = Arc::clone(&self.context);
        let path = path.as_cstring()?;

        task::spawn_blocking(move || unsafe {
            context
                .check_retcode_ret(libnfs::nfs_access2(context.0, path.as_ptr()))
                .map(AccessFlags::from_bits_truncate)
        })
        .await?
    }

    pub async fn mkdir<P: AsRef<Path>>(&self, path: P, mode: Mode) -> crate::Result<()> {
        let context = Arc::clone(&self.context);
        let path = path.as_cstring()?;

        task::spawn_blocking(move || unsafe {
            context.check_retcode(libnfs::nfs_mkdir2(
                context.0,
                path.as_ptr(),
                mode.bits() as i32,
            ))
        })
        .await?
    }

    pub async fn open<P: AsRef<Path>>(
        &self,
        path: P,
        flags: OFlag,
        mode: Mode,
    ) -> crate::Result<crate::File> {
        let context = Arc::clone(&self.context);
        let path = path.as_cstring()?;

        task::spawn_blocking(move || unsafe {
            let mut file = mem::MaybeUninit::uninit();

            context.check_retcode(libnfs::nfs_open2(
                context.0,
                path.as_ptr(),
                flags.bits() as i32,
                mode.bits() as i32,
                file.as_mut_ptr(),
            ))?;

            Ok(crate::File::new(context, file.assume_init()))
        })
        .await?
    }

    pub async fn rmdir<P: AsRef<Path>>(&self, path: P) -> crate::Result<()> {
        let context = Arc::clone(&self.context);
        let path = path.as_cstring()?;

        task::spawn_blocking(move || unsafe {
            context.check_retcode(libnfs::nfs_rmdir(context.0, path.as_ptr()))
        })
        .await?
    }

    pub async fn stat<P: AsRef<Path>>(&self, path: P) -> crate::Result<crate::Stat> {
        let context = Arc::clone(&self.context);
        let path = path.as_cstring()?;

        task::spawn_blocking(move || unsafe {
            let mut stat = mem::MaybeUninit::uninit();

            context.check_retcode(libnfs::nfs_stat64(
                context.0,
                path.as_ptr(),
                stat.as_mut_ptr(),
            ))?;

            Ok(stat.assume_init())
        })
        .await?
    }

    pub async fn unlink<P: AsRef<Path>>(&self, path: P) -> crate::Result<()> {
        let context = Arc::clone(&self.context);
        let path = path.as_cstring()?;

        task::spawn_blocking(move || unsafe {
            context.check_retcode(libnfs::nfs_unlink(context.0, path.as_ptr()))
        })
        .await?
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        if !self.context.0.is_null() {
            unsafe { libnfs::nfs_destroy_context(self.context.0) };
        }
    }
}

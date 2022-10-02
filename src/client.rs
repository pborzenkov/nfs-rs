use crate::ToStringLossy;
use libnfs_sys as libnfs;
use std::{ffi::CString, io, sync::Arc};
use tokio::task;

struct Context(*mut libnfs::nfs_context);

impl Context {
    fn is_valid(&self) -> bool {
        !self.0.is_null()
    }

    fn get_last_error(&self) -> String {
        unsafe { libnfs::nfs_get_error(self.0) }.to_string_lossy()
    }

    fn check_retcode(&self, code: i32) -> crate::Result<()> {
        if code < 0 {
            Err(crate::error::nfs(
                self.get_last_error(),
                io::Error::from_raw_os_error(-code),
            ))
        } else {
            Ok(())
        }
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { libnfs::nfs_destroy_context(self.0) };
        }
    }
}

unsafe impl Send for Context {}
unsafe impl Sync for Context {}

struct Url(*const libnfs::nfs_url);

impl Url {
    fn is_valid(&self) -> bool {
        !self.0.is_null()
    }
}

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
            if !context.is_valid() {
                return Err(crate::error::nfs(
                    "can't initialize libnfs context",
                    io::ErrorKind::OutOfMemory,
                ));
            }

            let url = CString::new(url.into_url()?.as_str())
                .map_err(|e| crate::error::nfs("can't parse URL", e))?;
            let url = Url(libnfs::nfs_parse_url_dir(context.0, url.as_ptr()));
            if !url.is_valid() {
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
        let context = self.context.clone();

        task::spawn_blocking(move || unsafe {
            libnfs::nfs_mt_service_thread_stop(context.0);
            context.check_retcode(libnfs::nfs_umount(context.0))
        })
        .await?
    }
}

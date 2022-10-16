use libnfs_sys as libnfs;
use nix::unistd::Whence;
use std::ffi::c_void;
use std::future::Future;
use std::{
    io, mem,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};
use tokio::io::AsyncWrite;
use tokio::sync::Mutex;
use tokio::{
    io::{AsyncRead, ReadBuf},
    task::{self, JoinHandle},
};

use crate::buf::Buf;

struct Fh(*mut libnfs::nfsfh);

unsafe impl Send for Fh {}
unsafe impl Sync for Fh {}

pub struct File {
    context: Arc<crate::client::Context>,
    file: Arc<Fh>,

    inner: Mutex<Inner>,
}

struct Inner {
    state: State,

    last_write_err: Option<io::ErrorKind>,
}

enum State {
    Idle(Option<Buf>),
    Busy(JoinHandle<(Operation, Buf)>),
}

enum Operation {
    Read(io::Result<usize>),
    Write(io::Result<()>),
}

macro_rules! ready {
    ($e:expr $(,)?) => {
        match $e {
            std::task::Poll::Ready(t) => t,
            std::task::Poll::Pending => return std::task::Poll::Pending,
        }
    };
}

impl File {
    pub(crate) fn new(context: Arc<crate::client::Context>, file: *mut libnfs::nfsfh) -> File {
        File {
            context,
            file: Arc::new(Fh(file)),
            inner: Mutex::new(Inner {
                state: State::Idle(Some(Buf::with_capacity(0))),
                last_write_err: None,
            }),
        }
    }

    pub async fn stat(&self) -> crate::Result<crate::Stat> {
        let context = Arc::clone(&self.context);
        let file = Arc::clone(&self.file);

        task::spawn_blocking(move || unsafe {
            let mut stat = mem::MaybeUninit::uninit();

            context.check_retcode(libnfs::nfs_fstat64(context.0, file.0, stat.as_mut_ptr()))?;

            Ok(stat.assume_init())
        })
        .await?
    }

    pub async fn sync_all(&self) -> crate::Result<()> {
        let context = Arc::clone(&self.context);
        let file = Arc::clone(&self.file);

        task::spawn_blocking(move || unsafe {
            context.check_retcode(libnfs::nfs_fsync(context.0, file.0))
        })
        .await?
    }
}

// AsyncRead and AsyncWrite implementation is shamelessly stolen from Tokio.
impl AsyncRead for File {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        dst: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    if !buf.is_empty() {
                        buf.copy_to(dst);
                        *buf_cell = Some(buf);
                        return Poll::Ready(Ok(()));
                    }

                    buf.ensure_capacity_for(dst);

                    let context = Arc::clone(&me.context);
                    let file = Arc::clone(&me.file);

                    inner.state = State::Busy(task::spawn_blocking(move || unsafe {
                        let res = context
                            .check_retcode_ret(libnfs::nfs_read(
                                context.0,
                                file.0,
                                buf.len() as u64,
                                buf.mut_bytes().as_mut_ptr() as *mut c_void,
                            ))
                            .map(|r| r as usize)
                            .map_err(|e| e.into_io());

                        if let Ok(n) = res {
                            buf.truncate(n);
                        } else {
                            buf.clear();
                        }

                        (Operation::Read(res), buf)
                    }))
                }
                State::Busy(ref mut rx) => {
                    let (op, mut buf) = ready!(Pin::new(rx).poll(cx))?;

                    match op {
                        Operation::Read(Ok(_)) => {
                            buf.copy_to(dst);
                            inner.state = State::Idle(Some(buf));
                            return Poll::Ready(Ok(()));
                        }
                        Operation::Read(Err(e)) => {
                            assert!(buf.is_empty());

                            inner.state = State::Idle(Some(buf));
                            return Poll::Ready(Err(e));
                        }
                        Operation::Write(Ok(_)) => {
                            assert!(buf.is_empty());
                            inner.state = State::Idle(Some(buf));
                            continue;
                        }
                        Operation::Write(Err(e)) => {
                            assert!(inner.last_write_err.is_none());
                            inner.last_write_err = Some(e.kind());
                            inner.state = State::Idle(Some(buf));
                        }
                    }
                }
            }
        }
    }
}

impl AsyncWrite for File {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        src: &[u8],
    ) -> Poll<io::Result<usize>> {
        let me = self.get_mut();
        let inner = me.inner.get_mut();

        if let Some(e) = inner.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        loop {
            match inner.state {
                State::Idle(ref mut buf_cell) => {
                    let mut buf = buf_cell.take().unwrap();

                    let seek = if !buf.is_empty() {
                        Some(buf.discard_read())
                    } else {
                        None
                    };

                    let n = buf.copy_from(src);
                    let context = Arc::clone(&me.context);
                    let file = Arc::clone(&me.file);

                    inner.state = State::Busy(task::spawn_blocking(move || unsafe {
                        let mut cur_offset: u64 = 0;

                        if let Some(seek) = seek {
                            let res = context
                                .check_retcode(libnfs::nfs_lseek(
                                    context.0,
                                    file.0,
                                    seek,
                                    Whence::SeekCur as i32,
                                    &mut cur_offset as *mut u64,
                                ))
                                .map_err(|e| e.into_io());

                            if res.is_err() {
                                return (Operation::Write(res), buf);
                            }
                        }

                        let mut written: usize = 0;
                        while written < buf.len() {
                            match context
                                .check_retcode_ret(libnfs::nfs_write(
                                    context.0,
                                    file.0,
                                    (buf.len() - written) as u64,
                                    buf.mut_bytes()[written..].as_mut_ptr() as *mut c_void,
                                ))
                                .map_err(|e| e.into_io())
                            {
                                Ok(n) => written += n as usize,
                                Err(e) => {
                                    buf.clear();
                                    return (Operation::Write(Err(e)), buf);
                                }
                            };
                        }

                        buf.clear();
                        (Operation::Write(Ok(())), buf)
                    }));

                    return Poll::Ready(Ok(n));
                }
                State::Busy(ref mut rx) => {
                    let (op, buf) = ready!(Pin::new(rx).poll(cx))?;
                    inner.state = State::Idle(Some(buf));

                    match op {
                        Operation::Read(_) => {
                            continue;
                        }
                        Operation::Write(res) => {
                            res?;
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let inner = self.inner.get_mut();
        inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        self.poll_flush(cx)
    }
}

impl Inner {
    fn poll_flush(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        if let Some(e) = self.last_write_err.take() {
            return Poll::Ready(Err(e.into()));
        }

        let (op, buf) = match self.state {
            State::Idle(_) => return Poll::Ready(Ok(())),
            State::Busy(ref mut rx) => ready!(Pin::new(rx).poll(cx))?,
        };

        // The buffer is not used here
        self.state = State::Idle(Some(buf));

        match op {
            Operation::Read(_) => Poll::Ready(Ok(())),
            Operation::Write(res) => Poll::Ready(res),
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if !self.context.0.is_null() && !self.file.0.is_null() {
            let context = self.context.clone();
            let file = self.file.clone();

            unsafe { libnfs::nfs_close(context.0, file.0) };
        }
    }
}

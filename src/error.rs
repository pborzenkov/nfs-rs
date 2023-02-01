use std::{error::Error as StdError, fmt, io};
use tokio::task;

/// A `Result` alias where the `Err` is `nfs::Error`
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that may be returned by the library.
pub struct Error {
    inner: Box<Inner>,
}

type BoxError = Box<dyn StdError + Send + Sync>;

struct Inner {
    kind: Kind,
    source: Option<BoxError>,
}

impl Error {
    pub(crate) fn new<E>(kind: Kind, source: Option<E>) -> Error
    where
        E: Into<BoxError>,
    {
        Error {
            inner: Box::new(Inner {
                kind,
                source: source.map(Into::into),
            }),
        }
    }

    pub fn into_io(&self) -> io::Error {
        self.inner
            .source
            .as_ref()
            .and_then(|source| {
                source
                    .downcast_ref::<io::Error>()
                    .map(|e| io::Error::new(e.kind(), e.to_string()))
            })
            .unwrap_or_else(|| io::Error::new(io::ErrorKind::Other, self.to_string()))
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_struct("nfs::Error");

        builder.field("kind", &self.inner.kind);
        if let Some(ref source) = self.inner.source {
            builder.field("source", source);
        }

        builder.finish()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.inner.kind {
            Kind::Url => f.write_str("URL error")?,
            Kind::Nfs(msg) => write!(f, "NFS error: {msg}")?,
            Kind::Runtime => f.write_str("runtime error")?,
        };

        if let Some(e) = &self.inner.source {
            write!(f, ": {e}")?;
        }

        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.inner.source.as_ref().map(|err| &**err as _)
    }
}

impl From<task::JoinError> for Error {
    fn from(e: task::JoinError) -> Self {
        Error::new(Kind::Runtime, Some(e))
    }
}

pub(crate) fn url<E: Into<BoxError>>(e: E) -> Error {
    Error::new(Kind::Url, Some(e))
}

pub(crate) fn nfs<M: Into<String>, E: Into<io::Error>>(msg: M, e: E) -> Error {
    Error::new(Kind::Nfs(msg.into()), Some(e.into()))
}

#[derive(Debug)]
pub(crate) enum Kind {
    Url,
    Nfs(String),
    Runtime,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn io_not_found() {
        let err = nfs(
            "file not found",
            io::Error::new(io::ErrorKind::NotFound, "not found"),
        );
        assert_eq!(err.into_io().kind(), io::ErrorKind::NotFound)
    }

    #[test]
    fn non_io_error() {
        let err = url("invalid scheme");
        assert_eq!(err.into_io().kind(), io::ErrorKind::Other)
    }
}

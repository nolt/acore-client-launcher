use std::fmt;
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// Network failure: connection, TLS, timeout, broken transfer.
    Http(reqwest::Error),
    /// The server answered, but not with the file.
    HttpStatus {
        url: String,
        status: u16,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// The manifest is malformed or points outside the game folder.
    Manifest(String),
    /// A downloaded file does not match the manifest.
    HashMismatch {
        path: String,
    },
    SizeMismatch {
        path: String,
        expected: u64,
        actual: u64,
    },
    /// The player paused the downloads.
    Paused,
}

impl Error {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// Worth trying the same download again.
    pub(crate) fn is_transient(&self) -> bool {
        match self {
            Error::Http(_) | Error::HashMismatch { .. } | Error::SizeMismatch { .. } => true,
            Error::HttpStatus { status, .. } => *status >= 500 || *status == 408 || *status == 429,
            Error::Io { .. } | Error::Manifest(_) | Error::Paused => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Http(e) => write!(f, "network error: {e}"),
            Error::HttpStatus { url, status } => write!(f, "HTTP {status} for {url}"),
            Error::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Manifest(msg) => write!(f, "invalid manifest: {msg}"),
            Error::HashMismatch { path } => {
                write!(f, "{path}: downloaded file does not match the manifest hash")
            }
            Error::SizeMismatch {
                path,
                expected,
                actual,
            } => {
                write!(f, "{path}: expected {expected} bytes, got {actual}")
            }
            Error::Paused => write!(f, "paused"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Http(e) => Some(e),
            Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(e: reqwest::Error) -> Self {
        Error::Http(e)
    }
}

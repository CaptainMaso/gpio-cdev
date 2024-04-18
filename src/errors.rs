//! This module is deprecated and types are exported from the top-level of the crate
//!
//! In futures versions of the crate, this module will no longer be included in the crate.

use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IOError;

pub(crate) type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
}

#[derive(Debug)]
pub enum ErrorKind {
    Event(nix::Error),
    Io(IOError),
    Offset(u32),
    InvalidRequest(u32, u32),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self.kind {
            ErrorKind::Event(err) => write!(f, "Failed to read event: {}", err),
            ErrorKind::Io(err) => err.fmt(f),
            ErrorKind::InvalidRequest(n_lines, n_values) => write!(
                f,
                "Invalid request: {} values requested to be set but only {} lines are open",
                n_values, n_lines
            ),
            ErrorKind::Offset(offset) => write!(f, "Offset {} is out of range", offset),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match &self.kind {
            ErrorKind::Event(err) => Some(err),
            ErrorKind::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<IOError> for Error {
    fn from(err: IOError) -> Self {
        Self {
            kind: ErrorKind::Io(err),
        }
    }
}

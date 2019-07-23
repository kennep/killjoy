//! Custom exceptions.

use std::error::Error;
use std::fmt;

/// An error indicating that finding a configuration file failed.
#[derive(Debug)]
pub struct FindConfigFileError;

impl Error for FindConfigFileError {}

impl fmt::Display for FindConfigFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No configuration file found.")
    }
}

/// An error indicating that parsing a configuration file failed.
#[derive(Debug)]
pub struct ParseConfigFileError {
    pub msg: String,
}

impl Error for ParseConfigFileError {}

impl fmt::Display for ParseConfigFileError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.msg[..])
    }
}

/// An error indicating that a file path could not be parsed.
///
/// OS file paths are most commonly parsed as unicode.
#[derive(Debug)]
pub struct ParsePathError;

impl Error for ParsePathError {}

impl fmt::Display for ParsePathError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to convert file path to a unicode string.")
    }
}

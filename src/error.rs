//! Custom exceptions.

use std::error::Error;
use std::fmt;

/// An error indicating that a configuration file could not be found.
#[derive(Debug)]
pub struct ConfigFileNotFoundError;

impl Error for ConfigFileNotFoundError {}

impl fmt::Display for ConfigFileNotFoundError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "No configuration file found.")
    }
}

/// An error indicating that a configuration file could not be decoded.
#[derive(Debug)]
pub struct ConfigFileDecodeError {
    pub msg: String,
}

impl Error for ConfigFileDecodeError {}

impl fmt::Display for ConfigFileDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.msg[..])
    }
}

/// An error indicating that a file path could not be converted to a unicode string.
#[derive(Debug)]
pub struct PathToUnicodeError;

impl Error for PathToUnicodeError {}

impl fmt::Display for PathToUnicodeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Failed to convert file path to a unicode string.")
    }
}

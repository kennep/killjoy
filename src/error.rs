// Custom exceptions.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};

// An error indicating that finding a configuration file failed.
#[derive(Debug)]
pub struct FindConfigFileError;

impl Display for FindConfigFileError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "No configuration file found.")
    }
}

impl Error for FindConfigFileError {}

// An error indicating that parsing a configuration file failed.
#[derive(Debug)]
pub struct ParseConfigFileError {
    pub msg: String,
}

impl Display for ParseConfigFileError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", &self.msg[..])
    }
}

impl Error for ParseConfigFileError {}

// An error indicating that a file path could not be parsed.
//
// OS file paths are most commonly parsed as unicode.
#[derive(Debug)]
pub struct ParsePathError;

impl Display for ParsePathError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "Failed to convert file path to a unicode string.")
    }
}

impl Error for ParsePathError {}

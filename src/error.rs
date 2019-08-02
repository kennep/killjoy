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

// An error indicating that parsing an object to an ActiveState failed.
#[derive(Debug)]
pub struct ParseAsActiveStateError {
    pub msg: String,
}

impl Display for ParseAsActiveStateError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", &self.msg[..])
    }
}

impl Error for ParseAsActiveStateError {}

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

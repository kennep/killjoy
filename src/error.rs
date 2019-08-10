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

// Like dbus::Error, but implements Send.
//
// This isn't a great way to handle errors. Doing this means that we don't know what went wrong,
// except by inspecting the `msg` field. It would be nicer if e.g. we had an enum with variants like
// ManagerUnitNew, ManagerUnitRemoved, etc, so that the type system could communicate what went
// wrong.
#[derive(Debug)]
pub struct MyDBusError {
    pub msg: String,
}

impl MyDBusError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

impl Display for MyDBusError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", self.msg)
    }
}

impl Error for MyDBusError {}

// Custom exceptions.

use std::error::Error;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io::Error as IOError;
use std::num::ParseIntError;
use std::str::Utf8Error;

use crate::unit::ActiveState;
use dbus::Error as ExternDBusError;

use regex::Error as RegexError;
use serde_json::error::Error as SerdeJsonError;

// An error in src/main.rs.
//
// Errors that bubble up to src/main.rs are top level errors.
#[derive(Debug)]
pub enum TopLevelError {
    // A wrapper.
    DBusError(DBusError),

    // The --loop-timeout argument is absent.
    GetLoopTimeoutArg,

    // If one calls std::thead::ThreadHandle::join and the referenced thread panics, the value
    // passed to panic! is returned in a Box. This variant references that value.
    MonitoringThreadPanicked(Box<dyn std::any::Any + std::marker::Send>),

    // The --loop-timeout argument is unparseable, i.e. some_str.parse::<u32>() failed.
    ParseLoopTimeoutArg(ParseIntError),

    // A wrapper.
    SettingsFileError(SettingsFileError),

    // This *should* always be Some(subcmd), but clap doesn't guarantee it.
    UnexpectedSubcommand(Option<String>),
}

impl Display for TopLevelError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            TopLevelError::DBusError(err) => write!(f, "{}", err),
            TopLevelError::GetLoopTimeoutArg => {
                write!(f, "Failed to get --loop-timeout argument. A default should've been set in the argument parser.")
            }
            TopLevelError::MonitoringThreadPanicked(err) => {
                write!(f, "A monitoring thread panicked. Source: {:?}", err)
            }
            TopLevelError::ParseLoopTimeoutArg(err) => {
                write!(f, "Failed to parse --loop-timeout argument: {:?}", err)
            }
            TopLevelError::SettingsFileError(err) => write!(f, "{}", err),
            TopLevelError::UnexpectedSubcommand(subcmd_opt) => match subcmd_opt {
                Some(subcmd) => write!(f, "An unexpected subcommand was encountered: {}", subcmd),
                None => write!(f, "An unexpected subcommand was encountered."),
            },
        }
    }
}

impl Error for TopLevelError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            TopLevelError::DBusError(err) => Some(err),
            TopLevelError::GetLoopTimeoutArg => None,
            TopLevelError::MonitoringThreadPanicked(_) => None,
            TopLevelError::ParseLoopTimeoutArg(err) => Some(err),
            TopLevelError::SettingsFileError(err) => Some(err),
            TopLevelError::UnexpectedSubcommand(_) => None,
        }
    }
}

// An error used when working with a settings file.
#[derive(Debug)]
pub enum SettingsFileError {
    DeserializationFailed(SerdeJsonError),
    FileNotFound(String),
    FileNotReadable(IOError),
    InvalidActiveState(String),
    InvalidBusName(String),
    InvalidBusType(String),
    InvalidExpressionType(String),
    InvalidNotifier(String),
    InvalidRegex(RegexError),
}

impl Display for SettingsFileError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            SettingsFileError::DeserializationFailed(err) => {
                write!(f, "Failed to deserialize the settings file: {}", err,)
            }
            SettingsFileError::FileNotFound(path) => write!(
                f,
                "Failed to find a configuration file in $XDG_CONFIG_HOME or $XDG_CONFIG_DIRS with path {}",
                path
            ),
            SettingsFileError::FileNotReadable(err) => {
                write!(f, "Failed to read settings file: {}", err)
            }
            SettingsFileError::InvalidActiveState(as_str) => {
                write!(f, "Found invalid active state: {}", as_str)
            }
            SettingsFileError::InvalidBusName(bn_str) => {
                write!(f, "Found invalid bus name: {}", bn_str)
            }
            SettingsFileError::InvalidBusType(bt_str) => {
                write!(f, "Found invalid bus type: {}", bt_str)
            }
            SettingsFileError::InvalidExpressionType(et_str) => {
                write!(f, "Found invalid expression type: {}", et_str)
            }
            SettingsFileError::InvalidRegex(err) => {
                write!(f, "Found invalid regular expression: {}", err)
            }
            SettingsFileError::InvalidNotifier(notifier) => {
                write!(f, "Rule references non-existent notifier: {}", notifier)
            }
        }
    }
}

impl Error for SettingsFileError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SettingsFileError::DeserializationFailed(err) => Some(err),
            SettingsFileError::FileNotReadable(err) => Some(err),
            SettingsFileError::InvalidRegex(err) => Some(err),
            _ => None,
        }
    }
}

// An error indicating that parsing an object to an ActiveState failed.
#[derive(Debug)]
pub struct ParseAsActiveStateError {
    msg: String,
}

impl ParseAsActiveStateError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}

impl Display for ParseAsActiveStateError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        write!(f, "{}", &self.msg[..])
    }
}

impl Error for ParseAsActiveStateError {}

// Like dbus::Error, but with more granular semantics, and implements Send.
#[derive(Debug)]
pub enum DBusError {
    AddMatch(String, ExternDBusError),
    CallOrgFreedesktopDBusPropertiesGetAll(ExternDBusError),
    CallOrgFreedesktopSystemd1ManagerGetUnit(ExternDBusError),
    CallOrgFreedesktopSystemd1ManagerListUnits(ExternDBusError),
    CallOrgFreedesktopSystemd1ManagerSubscribe(ExternDBusError),
    CastBusNameToStr(Utf8Error),
    CastOrgFreedesktopSystemd1UnitActiveState,
    CastOrgFreedesktopSystemd1UnitId,
    CastOrgFreedesktopSystemd1UnitTimestamp(&'static str),
    CastStrToPath(String),
    ConnectToBus(ExternDBusError),
    DecodeOrgFreedesktopSystemd1UnitActiveState(ParseAsActiveStateError),
    GetOrgFreedesktopSystemd1UnitId(ExternDBusError),
    MessageLacksPath,
    PropertiesLacksActiveState,
    PropertiesLacksTimestamp(ActiveState, &'static str),
    RemoveMatch(String, ExternDBusError),
}

impl Display for DBusError {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            DBusError::AddMatch(match_str, source) => {
                write!(f, "Failed to add match string '{}': {}", match_str, source)
            }
            DBusError::CallOrgFreedesktopDBusPropertiesGetAll(source) => {
                write!(f, "Failed to call org.freedesktop.DBus.Properties.GetAll: {}", source)
            }
            DBusError::CallOrgFreedesktopSystemd1ManagerGetUnit(source) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.GetUnit: {}", source)
            }
            DBusError::CallOrgFreedesktopSystemd1ManagerListUnits(source) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.ListUnits: {}", source)
            }
            DBusError::CallOrgFreedesktopSystemd1ManagerSubscribe(source) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.Subscribe: {}", source)
            }
            DBusError::CastBusNameToStr(source) => {
                write!(f, "Failed to cast bus name to UTF-8 string: {}", source)
            }
            DBusError::CastOrgFreedesktopSystemd1UnitTimestamp(timestamp_key) => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.{} to a u64.", timestamp_key)
            }
            DBusError::CastOrgFreedesktopSystemd1UnitActiveState => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.ActiveState to a string.")
            }
            DBusError::CastOrgFreedesktopSystemd1UnitId => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.Id to a string.")
            }
            DBusError::CastStrToPath(source) => {
                write!(f, "{}", source)
            }
            DBusError::ConnectToBus(source) => {
                write!(f, "Failed to connect to D-Bus bus. Cause: {}", source)
            }
            DBusError::DecodeOrgFreedesktopSystemd1UnitActiveState(source) => {
                write!(f, "Failed to decode org.freedesktop.systemd1.Unit.ActiveState: {}", source)
            }
            DBusError::GetOrgFreedesktopSystemd1UnitId(source) => {
                write!(f, "Failed to get org.freedesktop.systemd1.Unit.Id for: {}", source)
            }
            DBusError::MessageLacksPath => {
                write!(f, "Failed to get path from message headers.")
            }
            DBusError::PropertiesLacksActiveState => {
                write!(f, "A unit's properties lacks the ActiveState property.")
            }
            DBusError::PropertiesLacksTimestamp(active_state, timestamp_key) => write!(
                f,
                "A unit has entered the {:?} state, but that unit's properties lack a timestamp named '{}'.",
                active_state, timestamp_key
            ),
            DBusError::RemoveMatch(match_str, source) => {
                write!(f, "Failed to remove match string '{}': {}", match_str, source)
            }
        }
    }
}

impl Error for DBusError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            DBusError::AddMatch(_, source) => Some(source),
            DBusError::CallOrgFreedesktopDBusPropertiesGetAll(source) => Some(source),
            DBusError::CallOrgFreedesktopSystemd1ManagerGetUnit(source) => Some(source),
            DBusError::CallOrgFreedesktopSystemd1ManagerListUnits(source) => Some(source),
            DBusError::CallOrgFreedesktopSystemd1ManagerSubscribe(source) => Some(source),
            DBusError::CastBusNameToStr(source) => Some(source),
            DBusError::ConnectToBus(source) => Some(source),
            DBusError::GetOrgFreedesktopSystemd1UnitId(source) => Some(source),
            DBusError::RemoveMatch(_, source) => Some(source),
            _ => None,
        }
    }
}

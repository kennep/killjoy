// Custom exceptions.

use std::error::Error as StdError;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::io::Error as IOError;
use std::num::ParseIntError;
use std::str::Utf8Error;

use crate::unit::ActiveState;
use dbus::Error as ExternDBusError;

use regex::Error as RegexError;
use serde_json::error::Error as SerdeJsonError;

// This application's error type.
#[derive(Debug)]
pub enum Error {
    MissingLoopTimeoutArg,
    MonitoringThreadPanicked(Box<dyn std::any::Any + std::marker::Send>),
    ParseLoopTimeoutArg(ParseIntError),
    UnexpectedSubcommand(Option<String>), // Typically Some(subcmd), but clap doesn't guarantee it.

    SettingsFileDeserializationFailed(SerdeJsonError),
    SettingsFileNotFound(String),
    SettingsFileNotReadable(IOError),

    InvalidActiveState(String),
    InvalidBusName(String),
    InvalidBusType(String),
    InvalidExpressionType(String),
    InvalidNotifier(String),
    InvalidRegex(RegexError),

    // Like dbus::Error, but with more granular semantics, and implements Send.
    AddSignalMatch(String, ExternDBusError),
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
    GetOrgFreedesktopSystemd1UnitId(ExternDBusError),
    MessageLacksPath,
    PropertiesLacksActiveState,
    PropertiesLacksTimestamp(ActiveState, &'static str),
    RemoveSignalMatch(String, ExternDBusError),
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> FmtResult {
        match self {
            Error::MissingLoopTimeoutArg => {
                write!(f, "Failed to get --loop-timeout argument. A default should've been set in the argument parser.")
            }
            Error::MonitoringThreadPanicked(err) => {
                write!(f, "A monitoring thread panicked. Source: {:?}", err)
            }
            Error::ParseLoopTimeoutArg(err) => {
                write!(f, "Failed to parse --loop-timeout argument: {:?}", err)
            }
            Error::UnexpectedSubcommand(subcmd_opt) => match subcmd_opt {
                Some(subcmd) => write!(f, "An unexpected subcommand was encountered: {}", subcmd),
                None => write!(f, "An unexpected subcommand was encountered."),
            }

            Error::SettingsFileDeserializationFailed(err) => {
                write!(f, "Failed to deserialize the settings file: {}", err,)
            }
            Error::SettingsFileNotFound(path) => write!(
                f,
                "Failed to find a configuration file in $XDG_CONFIG_HOME or $XDG_CONFIG_DIRS with path {}",
                path
            ),
            Error::SettingsFileNotReadable(err) => {
                write!(f, "Failed to read settings file: {}", err)
            }

            Error::InvalidActiveState(as_str) => {
                write!(f, "Found invalid active state: {}", as_str)
            }
            Error::InvalidBusName(bn_str) => {
                write!(f, "Found invalid bus name: {}", bn_str)
            }
            Error::InvalidBusType(bt_str) => {
                write!(f, "Found invalid bus type: {}", bt_str)
            }
            Error::InvalidExpressionType(et_str) => {
                write!(f, "Found invalid expression type: {}", et_str)
            }
            Error::InvalidRegex(err) => {
                write!(f, "Found invalid regular expression: {}", err)
            }
            Error::InvalidNotifier(notifier) => {
                write!(f, "Rule references non-existent notifier: {}", notifier)
            }

            Error::AddSignalMatch(match_str, source) => {
                write!(f, "Failed to add match string '{}': {}", match_str, source)
            }
            Error::CallOrgFreedesktopDBusPropertiesGetAll(source) => {
                write!(f, "Failed to call org.freedesktop.DBus.Properties.GetAll: {}", source)
            }
            Error::CallOrgFreedesktopSystemd1ManagerGetUnit(source) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.GetUnit: {}", source)
            }
            Error::CallOrgFreedesktopSystemd1ManagerListUnits(source) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.ListUnits: {}", source)
            }
            Error::CallOrgFreedesktopSystemd1ManagerSubscribe(source) => {
                write!(f, "Failed to call org.freedesktop.systemd1.Manager.Subscribe: {}", source)
            }
            Error::CastBusNameToStr(source) => {
                write!(f, "Failed to cast bus name to UTF-8 string: {}", source)
            }
            Error::CastOrgFreedesktopSystemd1UnitTimestamp(timestamp_key) => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.{} to a u64.", timestamp_key)
            }
            Error::CastOrgFreedesktopSystemd1UnitActiveState => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.ActiveState to a string.")
            }
            Error::CastOrgFreedesktopSystemd1UnitId => {
                write!(f, "Failed to cast org.freedesktop.systemd1.Unit.Id to a string.")
            }
            Error::CastStrToPath(source) => {
                write!(f, "{}", source)
            }
            Error::ConnectToBus(source) => {
                write!(f, "Failed to connect to D-Bus bus. Cause: {}", source)
            }
            Error::GetOrgFreedesktopSystemd1UnitId(source) => {
                write!(f, "Failed to get org.freedesktop.systemd1.Unit.Id for: {}", source)
            }
            Error::MessageLacksPath => {
                write!(f, "Failed to get path from message headers.")
            }
            Error::PropertiesLacksActiveState => {
                write!(f, "A unit's properties lacks the ActiveState property.")
            }
            Error::PropertiesLacksTimestamp(active_state, timestamp_key) => write!(
                f,
                "A unit has entered the {:?} state, but that unit's properties lack a timestamp named '{}'.",
                active_state, timestamp_key
            ),
            Error::RemoveSignalMatch(match_str, source) => {
                write!(f, "Failed to remove match string '{}': {}", match_str, source)
            }
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Error::MissingLoopTimeoutArg => None,
            Error::MonitoringThreadPanicked(_) => None,
            Error::ParseLoopTimeoutArg(err) => Some(err),
            Error::UnexpectedSubcommand(_) => None,

            Error::SettingsFileDeserializationFailed(err) => Some(err),
            Error::SettingsFileNotFound(_) => None,
            Error::SettingsFileNotReadable(err) => Some(err),

            Error::InvalidActiveState(_) => None,
            Error::InvalidBusName(_) => None,
            Error::InvalidBusType(_) => None,
            Error::InvalidExpressionType(_) => None,
            Error::InvalidNotifier(_) => None,
            Error::InvalidRegex(err) => Some(err),

            // To be flattened.
            Error::AddSignalMatch(_, err) => Some(err),
            Error::CallOrgFreedesktopDBusPropertiesGetAll(err) => Some(err),
            Error::CallOrgFreedesktopSystemd1ManagerGetUnit(err) => Some(err),
            Error::CallOrgFreedesktopSystemd1ManagerListUnits(err) => Some(err),
            Error::CallOrgFreedesktopSystemd1ManagerSubscribe(err) => Some(err),
            Error::CastBusNameToStr(err) => Some(err),
            Error::CastOrgFreedesktopSystemd1UnitActiveState => None,
            Error::CastOrgFreedesktopSystemd1UnitId => None,
            Error::CastOrgFreedesktopSystemd1UnitTimestamp(_) => None,
            Error::CastStrToPath(_) => None,
            Error::ConnectToBus(err) => Some(err),
            Error::GetOrgFreedesktopSystemd1UnitId(err) => Some(err),
            Error::MessageLacksPath => None,
            Error::PropertiesLacksActiveState => None,
            Error::PropertiesLacksTimestamp(_, _) => None,
            Error::RemoveSignalMatch(_, err) => Some(err),
        }
    }
}

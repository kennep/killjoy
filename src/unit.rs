// Logic for representing units.

use std::convert::TryFrom;
use std::fmt::{Display, Formatter, Result as FmtResult};

use crate::error::ParseAsActiveStateError;

// The possible values for a unit's `ActiveState` attribute.
//
// Systemd's D-Bus API provides units' ActiveState attribute as a string. This enum exists so that
// states may be represented internally in a more efficient and type-safe manner.
//
// For conceptual information on the ActiveState property:
//
// *   Search for "ActiveState" in [The D-Bus API of systemd/PID
//     1](https://www.freedesktop.org/wiki/Software/systemd/dbus/)
// *   Read the "CONCEPTS" section in systemd(1).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ActiveState {
    Activating,
    Active,
    Deactivating,
    Failed,
    Inactive,
}

impl Display for ActiveState {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        let msg = match self {
            ActiveState::Activating => "activating",
            ActiveState::Active => "active",
            ActiveState::Deactivating => "deactivating",
            ActiveState::Failed => "failed",
            ActiveState::Inactive => "inactive",
        };
        write!(f, "{}", msg)
    }
}

// Useful when reading from a configuration file.
impl TryFrom<&str> for ActiveState {
    type Error = ParseAsActiveStateError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "activating" => Ok(ActiveState::Activating),
            "active" => Ok(ActiveState::Active),
            "deactivating" => Ok(ActiveState::Deactivating),
            "failed" => Ok(ActiveState::Failed),
            "inactive" => Ok(ActiveState::Inactive),
            _ => Err(ParseAsActiveStateError {
                msg: format!("Failed to parse string as ActiveState: {}", value,),
            }),
        }
    }
}

// Useful when writing to a bus or configuration file.
impl From<ActiveState> for String {
    fn from(value: ActiveState) -> String {
        match value {
            ActiveState::Activating => "activating".to_string(),
            ActiveState::Active => "active".to_string(),
            ActiveState::Deactivating => "deactivating".to_string(),
            ActiveState::Failed => "failed".to_string(),
            ActiveState::Inactive => "inactive".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct UnitStateMachine {
    active_state: ActiveState,
    timestamp: u64,
}

impl UnitStateMachine {
    // Initialize the state machine's attributes and call `on_change()`.
    pub fn new<T>(active_state: ActiveState, timestamp: u64, on_change: &T) -> Self
    where
        T: Fn(&UnitStateMachine, Option<ActiveState>),
    {
        let usm = UnitStateMachine {
            active_state,
            timestamp,
        };
        on_change(&usm, None);
        usm
    }

    // Optionally update the state machine's attributes and call `on_change()`.
    //
    // If the given `timestamp` is newer than the one currently in the state machine, then update
    // the state machine's attributes. If the `active_state` change, call `on_change()`.
    pub fn update<T>(&mut self, active_state: ActiveState, timestamp: u64, on_change: &T)
    where
        T: Fn(&UnitStateMachine, Option<ActiveState>),
    {
        if self.timestamp < timestamp {
            self.timestamp = timestamp;
            if self.active_state != active_state {
                let old_state = self.active_state;
                self.active_state = active_state;
                on_change(&self, Some(old_state));
            }
        }
    }

    pub fn active_state(&self) -> ActiveState {
        self.active_state
    }

    pub fn timestamp(&self) -> u64 {
        self.timestamp
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pass a unit state and a timestamp.
    #[test]
    fn test_usm_new() {
        let on_change = |_: &UnitStateMachine, _: Option<ActiveState>| {};
        let usm = UnitStateMachine::new(ActiveState::Failed, 10, &on_change);
        assert_eq!(usm.active_state, ActiveState::Failed);
        assert_eq!(usm.timestamp, 10);
    }

    // Unsuccessfully update the state machine.
    #[test]
    fn test_usm_update_v1() {
        let on_change = |_: &UnitStateMachine, _: Option<ActiveState>| {};
        let mut usm = UnitStateMachine::new(ActiveState::Inactive, 25, &on_change);

        usm.update(ActiveState::Activating, 24, &on_change);
        assert_eq!(usm.active_state, ActiveState::Inactive);
        assert_eq!(usm.timestamp, 25);

        usm.update(ActiveState::Active, 25, &on_change);
        assert_eq!(usm.active_state, ActiveState::Inactive);
        assert_eq!(usm.timestamp, 25);
    }

    // Successfully update the state machine.
    #[test]
    fn test_usm_update_v2() {
        let on_change = |_: &UnitStateMachine, _: Option<ActiveState>| {};
        let mut usm = UnitStateMachine::new(ActiveState::Inactive, 25, &on_change);

        usm.update(ActiveState::Activating, 26, &on_change);
        assert_eq!(usm.active_state, ActiveState::Activating);
        assert_eq!(usm.timestamp, 26);

        usm.update(ActiveState::Active, 27, &on_change);
        assert_eq!(usm.active_state, ActiveState::Active);
        assert_eq!(usm.timestamp, 27);
    }

    // Convert "activating" to an ActiveState.
    #[test]
    fn test_active_state_from_activating() {
        let active_state = ActiveState::try_from("activating").unwrap();
        assert_eq!(active_state, ActiveState::Activating);
    }

    // Convert "active" to an ActiveState.
    #[test]
    fn test_active_state_from_active() {
        let active_state = ActiveState::try_from("active").unwrap();
        assert_eq!(active_state, ActiveState::Active);
    }

    // Convert "deactivating" to an ActiveState.
    #[test]
    fn test_active_state_from_deactivating() {
        let active_state = ActiveState::try_from("deactivating").unwrap();
        assert_eq!(active_state, ActiveState::Deactivating);
    }

    // Convert "failed" to an ActiveState.
    #[test]
    fn test_active_state_from_failed() {
        let active_state = ActiveState::try_from("failed").unwrap();
        assert_eq!(active_state, ActiveState::Failed);
    }

    // Convert "inactive" to an ActiveState.
    #[test]
    fn test_active_state_from_inactive() {
        let active_state = ActiveState::try_from("inactive").unwrap();
        assert_eq!(active_state, ActiveState::Inactive);
    }

    // Convert some other string to an ActiveState. (It should fail.)
    #[test]
    fn test_active_state_from_other() {
        match ActiveState::try_from("foo") {
            Ok(_) => panic!("Conversion should have failed."),
            Err(_) => {}
        }
    }

    #[test]
    fn test_active_state_display() {
        let displayed = format!("{}", ActiveState::Deactivating);
        assert_eq!(&displayed[..], "deactivating");
    }

    #[test]
    // Create a String from an arbitrary ActiveState.
    fn test_string_from_active_state() {
        assert_eq!(String::from(ActiveState::Deactivating), "deactivating");
    }
}

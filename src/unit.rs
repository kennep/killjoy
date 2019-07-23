// Logic for representing units.

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

#[derive(Debug)]
pub struct UnitStateMachine {
    active_state: ActiveState,
    timestamp: u64,
}

impl UnitStateMachine {
    // Initialize the state machine's attributes and call `on_change()`.
    pub fn new<T>(active_state: ActiveState, timestamp: u64, on_change: &T) -> Self
    where
        T: Fn(&UnitStateMachine),
    {
        let usm = UnitStateMachine {
            active_state,
            timestamp,
        };
        on_change(&usm);
        usm
    }

    // Optionally update the state machine's attributes and call `on_change()`.
    //
    // If the given `timestamp` is newer than the one currently in the state machine, then update
    // the state machine's attributes. If the `active_state` change, call `on_change()`.
    pub fn update<T>(&mut self, active_state: ActiveState, timestamp: u64, on_change: &T)
    where
        T: Fn(&UnitStateMachine),
    {
        if self.timestamp < timestamp {
            self.timestamp = timestamp;
            if self.active_state != active_state {
                self.active_state = active_state;
                on_change(&self);
            }
        }
    }

    pub fn get_active_state(&self) -> ActiveState {
        self.active_state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Pass a unit state and a timestamp.
    #[test]
    fn test_usm_new() {
        let on_change = |_: &UnitStateMachine| {};
        let usm = UnitStateMachine::new(ActiveState::Failed, 10, &on_change);
        assert_eq!(usm.active_state, ActiveState::Failed);
        assert_eq!(usm.timestamp, 10);
    }

    // Unsuccessfully update the state machine.
    #[test]
    fn test_usm_update_v1() {
        let on_change = |_: &UnitStateMachine| {};
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
        let on_change = |_: &UnitStateMachine| {};
        let mut usm = UnitStateMachine::new(ActiveState::Inactive, 25, &on_change);

        usm.update(ActiveState::Activating, 26, &on_change);
        assert_eq!(usm.active_state, ActiveState::Activating);
        assert_eq!(usm.timestamp, 26);

        usm.update(ActiveState::Active, 27, &on_change);
        assert_eq!(usm.active_state, ActiveState::Active);
        assert_eq!(usm.timestamp, 27);
    }
}

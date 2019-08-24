// Logic for working with timestamps.

use crate::bus::UnitProps;
use crate::error::DBusError as CrateDBusError;
use crate::unit::ActiveState;

// Return the monotonic timestamp indicating when the given state was most recently entered.
pub fn get_monotonic_timestamp(
    active_state: ActiveState,
    unit_props: &UnitProps,
) -> Result<u64, CrateDBusError> {
    let timestamp_key: &'static str = get_monotonic_timestamp_key(active_state);
    unit_props
        .get(timestamp_key)
        .ok_or_else(|| CrateDBusError::PropertiesLacksTimestamp(active_state, timestamp_key))?
        .0
        .as_u64()
        .ok_or_else(|| CrateDBusError::CastOrgFreedesktopSystemd1UnitTimestamp(timestamp_key))
}

// Return name of the monotonic timestamp indicating when the given state was most recently entered.
fn get_monotonic_timestamp_key(active_state: ActiveState) -> &'static str {
    match active_state {
        ActiveState::Activating => "InactiveExitTimestampMonotonic",
        ActiveState::Active => "ActiveEnterTimestampMonotonic",
        ActiveState::Deactivating => "ActiveExitTimestampMonotonic",
        ActiveState::Failed => "InactiveEnterTimestampMonotonic",
        ActiveState::Inactive => "InactiveEnterTimestampMonotonic",
    }
}

// Return the realtime timestamp indicating when the given state was most recently entered.
pub fn get_realtime_timestamp(
    active_state: ActiveState,
    unit_props: &UnitProps,
) -> Result<u64, CrateDBusError> {
    let timestamp_key: &'static str = get_realtime_timestamp_key(active_state);
    unit_props
        .get(timestamp_key)
        .ok_or_else(|| CrateDBusError::PropertiesLacksTimestamp(active_state, timestamp_key))?
        .0
        .as_u64()
        .ok_or_else(|| CrateDBusError::CastOrgFreedesktopSystemd1UnitTimestamp(timestamp_key))
}

// Return name of the realtime timestamp indicating when the given state was most recently entered.
fn get_realtime_timestamp_key(active_state: ActiveState) -> &'static str {
    match active_state {
        ActiveState::Activating => "InactiveExitTimestamp",
        ActiveState::Active => "ActiveEnterTimestamp",
        ActiveState::Deactivating => "ActiveExitTimestamp",
        ActiveState::Failed => "InactiveEnterTimestamp",
        ActiveState::Inactive => "InactiveEnterTimestamp",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // get_monotonic_timestamp_key()
    #[test]
    fn test_get_monotonic_timestamp_key() {
        for act_st in vec![
            ActiveState::Activating,
            ActiveState::Active,
            ActiveState::Deactivating,
            ActiveState::Failed,
            ActiveState::Inactive,
        ] {
            assert!(get_monotonic_timestamp_key(act_st).contains("Monotonic"));
        }
    }

    // get_realtime_timestamp_key()
    #[test]
    fn test_get_realtime_timestamp_key() {
        for act_st in vec![
            ActiveState::Activating,
            ActiveState::Active,
            ActiveState::Deactivating,
            ActiveState::Failed,
            ActiveState::Inactive,
        ] {
            assert!(!get_realtime_timestamp_key(act_st).contains("Monotonic"));
        }
    }
}

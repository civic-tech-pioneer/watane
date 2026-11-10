//! A counter, stored as JSON in the private state of the mod.
wit_bindgen::generate!({ path: "../wit", world: "{{slug}}", generate_all });

use exports::mods::{{crate}}::api::Guest;
use mods::{{crate}}::changes;
use watane::host::state;

struct Mod;

impl Guest for Mod {
    fn add() -> u32 {
        let count = next(decode(state::get("count").as_deref()));
        state::put("count", &encode(count));
        changes::counted(count);
        count
    }

    fn get() -> u32 {
        decode(state::get("count").as_deref())
    }
}

export!(Mod);

/// Reads a stored count. A missing or broken value counts as 0.
fn decode(bytes: Option<&[u8]>) -> u32 {
    bytes.and_then(|b| serde_json::from_slice(b).ok()).unwrap_or(0)
}

fn encode(count: u32) -> Vec<u8> {
    serde_json::to_vec(&count).expect("a u32 always encodes")
}

/// The next count. It stays at the maximum instead of overflowing.
fn next(count: u32) -> u32 {
    count.saturating_add(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_missing_and_broken_values_as_zero() {
        assert_eq!(decode(None), 0);
        assert_eq!(decode(Some(b"41")), 41);
        assert_eq!(decode(Some(b"bad")), 0);
        assert_eq!(decode(Some(b"-1")), 0);
    }

    #[test]
    fn counts_up_and_stays_at_the_maximum() {
        assert_eq!(next(0), 1);
        assert_eq!(next(u32::MAX), u32::MAX);
        assert_eq!(encode(42), b"42");
    }
}

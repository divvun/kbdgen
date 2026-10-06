//! Which key events the text service eats (`tsf.keys.claim`).
//!
//! A key down is eaten exactly when the engine's action for it is not
//! `Pass`, and a key up exactly when its key down was eaten. While the
//! context is read-only, or the service is poisoned or inert, every key
//! passes.

use crate::keys::Role;

/// What to do with a key down before the engine is asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Pass, changing nothing.
    Ignore,
    /// Pass, and reset the context first (`tsf.edit.reset`).
    Reset,
    /// Eat without asking the engine.
    Eat,
    Engine,
}

/// Why every key passes, if one applies.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Blocked {
    pub poisoned: bool,
    /// No model is loaded for the active profile (`tsf.data.locate`).
    pub inert: bool,
    /// The context's `TS_SD_READONLY` dynamic flag is set.
    pub read_only: bool,
}

// [spec:kbdgen:req:tsf.keys.claim]
// [spec:kbdgen:req:tsf.component.panic]
pub fn route(role: &Role, blocked: Blocked) -> Route {
    if blocked.poisoned || blocked.inert {
        return Route::Ignore;
    }
    match role {
        Role::Own | Role::Modifier => Route::Ignore,
        Role::Other => Route::Reset,
        _ if blocked.read_only => Route::Reset,
        Role::AltGr => Route::Eat,
        Role::Engine(_) => Route::Engine,
    }
}

/// The physical keys whose key down was eaten and whose key up is due.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Claims {
    eaten: Vec<u16>,
}

impl Claims {
    pub fn down(&mut self, key: u16, eaten: bool) {
        self.eaten.retain(|&k| k != key);
        if eaten {
            self.eaten.push(key);
        }
    }

    /// Whether the key up of `key` is eaten, without consuming the claim,
    /// as `OnTestKeyUp` asks.
    pub fn holds(&self, key: u16) -> bool {
        self.eaten.contains(&key)
    }

    /// Whether the key up of `key` is eaten; consumes the claim.
    pub fn up(&mut self, key: u16) -> bool {
        let held = self.holds(key);
        self.eaten.retain(|&k| k != key);
        held
    }
}

#[cfg(test)]
mod tests {
    use kbd_engine::Key;

    use super::*;

    const OPEN: Blocked = Blocked {
        poisoned: false,
        inert: false,
        read_only: false,
    };

    // [spec:kbdgen:req:tsf.keys.claim/test]
    #[test]
    fn engine_keys_go_to_engine_when_open() {
        assert_eq!(route(&Role::Engine(Key::Scan(0x10)), OPEN), Route::Engine);
        assert_eq!(route(&Role::AltGr, OPEN), Route::Eat);
        assert_eq!(route(&Role::Other, OPEN), Route::Reset);
        assert_eq!(route(&Role::Modifier, OPEN), Route::Ignore);
        assert_eq!(route(&Role::Own, OPEN), Route::Ignore);
    }

    // [spec:kbdgen:req:tsf.keys.claim/test]
    // [spec:kbdgen:req:tsf.component.panic/test]
    #[test]
    fn poisoned_inert_or_read_only_pass_everything() {
        let roles = [
            Role::Engine(Key::Scan(0x10)),
            Role::AltGr,
            Role::Other,
            Role::Modifier,
        ];
        for blocked in [
            Blocked {
                poisoned: true,
                ..OPEN
            },
            Blocked {
                inert: true,
                ..OPEN
            },
            Blocked {
                read_only: true,
                ..OPEN
            },
        ] {
            for role in &roles {
                let routed = route(role, blocked);
                assert!(
                    matches!(routed, Route::Ignore | Route::Reset),
                    "{role:?} under {blocked:?} gives {routed:?}"
                );
            }
        }
    }

    // [spec:kbdgen:req:tsf.keys.claim/test]
    #[test]
    fn key_up_is_eaten_exactly_after_eaten_down() {
        let mut claims = Claims::default();
        claims.down(0x10, true);
        claims.down(0x11, false);
        assert!(claims.holds(0x10));
        assert!(claims.up(0x10));
        assert!(!claims.up(0x10));
        assert!(!claims.up(0x11));
        claims.down(0x12, true);
        claims.down(0x12, false);
        assert!(!claims.up(0x12));
    }
}

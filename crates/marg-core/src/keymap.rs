//! Key bindings per mode, and matching key presses (with counts and
//! multi-key sequences) against them.

use std::collections::BTreeMap;

use crate::command::Command;
use crate::keys::{KeyPress, Named, format_sequence, parse_sequence};

/// Value that removes a default binding in the config.
pub const UNBIND: &str = "nop";

/// Default Normal-mode bindings: (key sequence, command name).
pub const DEFAULT_NORMAL: &[(&str, &str)] = &[
    ("j", "scroll.down"),
    ("k", "scroll.up"),
    ("h", "scroll.left"),
    ("l", "scroll.right"),
    ("Down", "scroll.down"),
    ("Up", "scroll.up"),
    ("Left", "scroll.left"),
    ("Right", "scroll.right"),
    ("Ctrl-d", "scroll.half_down"),
    ("Ctrl-u", "scroll.half_up"),
    ("J", "page.next"),
    ("K", "page.prev"),
    ("Space", "page.next"),
    ("Shift-Space", "page.prev"),
    ("PageDown", "page.next"),
    ("PageUp", "page.prev"),
    ("gg", "page.first"),
    ("G", "page.last"),
    ("Home", "page.first"),
    ("End", "page.last"),
    ("+", "zoom.in"),
    ("-", "zoom.out"),
    ("=", "zoom.reset"),
    ("s", "zoom.fit_width"),
    ("a", "zoom.fit_page"),
    (":", "cmdline"),
    ("q", "quit"),
];

#[derive(Debug, Clone)]
struct Binding {
    keys: Vec<KeyPress>,
    command: Command,
}

#[derive(Debug, Clone, Default)]
pub struct Keymap {
    bindings: Vec<Binding>,
}

impl Keymap {
    /// The default Normal-mode keymap with the user's `[keys.normal]` table
    /// applied on top. Returns warnings for invalid entries and conflicts.
    pub fn normal(user: &BTreeMap<String, String>) -> (Self, Vec<String>) {
        let mut map = Self::default();
        let mut warnings = Vec::new();
        for (keys, cmd) in DEFAULT_NORMAL {
            map.bind(keys, cmd, false, &mut warnings);
        }
        for (keys, cmd) in user {
            map.bind(keys, cmd, true, &mut warnings);
        }
        (map, warnings)
    }

    fn bind(&mut self, keys: &str, command: &str, user: bool, warnings: &mut Vec<String>) {
        let Some(seq) = parse_sequence(keys) else {
            warnings.push(format!("keys: cannot parse key \"{keys}\""));
            return;
        };
        let command = command.trim();
        let parsed = if command == UNBIND {
            None
        } else {
            match Command::from_name(command) {
                Some(c) => Some(c),
                None => {
                    warnings.push(format!(
                        "keys: unknown command \"{command}\" for \"{keys}\""
                    ));
                    return;
                }
            }
        };
        // The new binding wins over any binding that is equal to it, or a
        // prefix of it, or has it as a prefix. Replacing an equal default
        // is the normal way to remap, so only real conflicts are reported.
        self.bindings.retain(|b| {
            let n = b.keys.len().min(seq.len());
            if b.keys[..n] != seq[..n] {
                return true;
            }
            if user && b.keys != seq {
                warnings.push(format!(
                    "keys: \"{keys}\" conflicts with \"{}\" ({}); \"{keys}\" wins",
                    format_sequence(&b.keys),
                    b.command
                ));
            }
            false
        });
        if let Some(command) = parsed {
            self.bindings.push(Binding { keys: seq, command });
        }
    }

    fn exact(&self, keys: &[KeyPress]) -> Option<&Command> {
        self.bindings
            .iter()
            .find(|b| b.keys == keys)
            .map(|b| &b.command)
    }

    fn has_prefix(&self, keys: &[KeyPress]) -> bool {
        self.bindings
            .iter()
            .any(|b| b.keys.len() > keys.len() && b.keys.starts_with(keys))
    }

    /// Whether a key is bound as the first key of some binding.
    fn starts_binding(&self, key: KeyPress) -> bool {
        self.bindings.iter().any(|b| b.keys[0] == key)
    }

    /// All bindings as (keys, command), for the help overlay.
    pub fn iter(&self) -> impl Iterator<Item = (String, &Command)> {
        self.bindings
            .iter()
            .map(|b| (format_sequence(&b.keys), &b.command))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Waiting for more keys (or count digits).
    Pending,
    Run {
        command: Command,
        count: Option<u32>,
    },
    /// The key matched nothing; pending keys were dropped.
    Unbound,
    /// Esc dropped a pending sequence or count.
    Cancelled,
}

const MAX_COUNT: u32 = 99_999;

/// Matches key presses against a keymap. Times are milliseconds from any
/// fixed point (the caller's clock).
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    keys: Vec<KeyPress>,
    count: Option<u32>,
    last_ms: u64,
}

impl Matcher {
    pub fn is_pending(&self) -> bool {
        !self.keys.is_empty() || self.count.is_some()
    }

    /// The pending count and keys, for the status bar.
    pub fn pending_text(&self) -> String {
        let count = self.count.map(|c| c.to_string()).unwrap_or_default();
        count + &format_sequence(&self.keys)
    }

    pub fn reset(&mut self) {
        self.keys.clear();
        self.count = None;
    }

    /// Drop a pending sequence that has timed out. Returns true if it did.
    pub fn expire(&mut self, now_ms: u64, timeout_ms: u64) -> bool {
        if self.is_pending() && now_ms.saturating_sub(self.last_ms) >= timeout_ms {
            self.reset();
            return true;
        }
        false
    }

    pub fn feed(&mut self, map: &Keymap, key: KeyPress, now_ms: u64, timeout_ms: u64) -> Outcome {
        self.expire(now_ms, timeout_ms);
        self.last_ms = now_ms;
        if key == KeyPress::named(Named::Esc) && self.is_pending() {
            self.reset();
            return Outcome::Cancelled;
        }
        // Digits are a count unless a binding starts with that digit; a
        // leading 0 is never a count.
        if self.keys.is_empty()
            && let Some(d) = key.digit()
            && (self.count.is_some() || d != 0)
            && !map.starts_binding(key)
        {
            let c = self.count.unwrap_or(0).saturating_mul(10).saturating_add(d);
            self.count = Some(c.min(MAX_COUNT));
            return Outcome::Pending;
        }
        self.keys.push(key);
        if let Some(command) = map.exact(&self.keys) {
            let out = Outcome::Run {
                command: command.clone(),
                count: self.count,
            };
            self.reset();
            return out;
        }
        if map.has_prefix(&self.keys) {
            return Outcome::Pending;
        }
        self.reset();
        Outcome::Unbound
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Keymap {
        let (map, warnings) = Keymap::normal(&BTreeMap::new());
        assert!(warnings.is_empty(), "{warnings:?}");
        map
    }

    fn feed_str(m: &mut Matcher, map: &Keymap, s: &str) -> Vec<Outcome> {
        s.chars()
            .map(|c| m.feed(map, KeyPress::char(c), 0, 1000))
            .collect()
    }

    fn run(command: Command, count: Option<u32>) -> Outcome {
        Outcome::Run { command, count }
    }

    #[test]
    fn defaults_parse_without_conflicts() {
        let map = defaults();
        assert_eq!(map.iter().count(), DEFAULT_NORMAL.len());
    }

    #[test]
    fn single_key_and_sequence() {
        let map = defaults();
        let mut m = Matcher::default();
        assert_eq!(
            feed_str(&mut m, &map, "j"),
            [run(Command::ScrollDown, None)]
        );
        assert_eq!(
            feed_str(&mut m, &map, "gg"),
            [Outcome::Pending, run(Command::PageFirst, None)]
        );
        assert_eq!(m.pending_text(), "");
    }

    #[test]
    fn counts() {
        let map = defaults();
        let mut m = Matcher::default();
        assert_eq!(
            feed_str(&mut m, &map, "42G"),
            [
                Outcome::Pending,
                Outcome::Pending,
                run(Command::PageLast, Some(42))
            ]
        );
        assert_eq!(
            feed_str(&mut m, &map, "5g"),
            [Outcome::Pending, Outcome::Pending]
        );
        assert_eq!(m.pending_text(), "5g");
        assert_eq!(
            feed_str(&mut m, &map, "g"),
            [run(Command::PageFirst, Some(5))]
        );
        // A leading zero is not a count (and 0 is unbound here).
        assert_eq!(feed_str(&mut m, &map, "0"), [Outcome::Unbound]);
    }

    #[test]
    fn unbound_key_drops_pending() {
        let map = defaults();
        let mut m = Matcher::default();
        assert_eq!(
            feed_str(&mut m, &map, "3gx"),
            [Outcome::Pending, Outcome::Pending, Outcome::Unbound]
        );
        assert!(!m.is_pending());
    }

    #[test]
    fn sequence_times_out() {
        let map = defaults();
        let mut m = Matcher::default();
        assert_eq!(m.feed(&map, KeyPress::char('g'), 0, 1000), Outcome::Pending);
        assert!(!m.expire(999, 1000));
        assert_eq!(
            m.feed(&map, KeyPress::char('g'), 1500, 1000),
            Outcome::Pending
        );
        assert!(m.expire(2500, 1000));
        assert!(!m.is_pending());
    }

    #[test]
    fn esc_cancels_pending() {
        let map = defaults();
        let mut m = Matcher::default();
        feed_str(&mut m, &map, "2g");
        assert_eq!(
            m.feed(&map, KeyPress::named(Named::Esc), 0, 1000),
            Outcome::Cancelled
        );
        assert!(!m.is_pending());
    }

    #[test]
    fn modified_keys() {
        let map = defaults();
        let mut m = Matcher::default();
        assert_eq!(
            m.feed(&map, KeyPress::char('d').with_ctrl(), 0, 1000),
            run(Command::HalfPageDown, None)
        );
    }

    #[test]
    fn user_remaps_and_unbinds() {
        let user = BTreeMap::from([
            ("j".to_string(), "page.next".to_string()),
            ("q".to_string(), UNBIND.to_string()),
        ]);
        let (map, warnings) = Keymap::normal(&user);
        assert!(warnings.is_empty(), "{warnings:?}");
        let mut m = Matcher::default();
        assert_eq!(feed_str(&mut m, &map, "j"), [run(Command::PageNext, None)]);
        assert_eq!(feed_str(&mut m, &map, "q"), [Outcome::Unbound]);
    }

    #[test]
    fn prefix_conflicts_are_reported_and_later_wins() {
        let user = BTreeMap::from([("g".to_string(), "zoom.in".to_string())]);
        let (map, warnings) = Keymap::normal(&user);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("\"gg\""));
        let mut m = Matcher::default();
        assert_eq!(feed_str(&mut m, &map, "g"), [run(Command::ZoomIn, None)]);
    }

    #[test]
    fn bad_entries_are_reported() {
        let user = BTreeMap::from([
            ("Bogus-x y".to_string(), "zoom.in".to_string()),
            ("x".to_string(), "no.such".to_string()),
        ]);
        let (_, warnings) = Keymap::normal(&user);
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn bound_digit_is_not_a_count() {
        let user = BTreeMap::from([("1".to_string(), "zoom.in".to_string())]);
        let (map, _) = Keymap::normal(&user);
        let mut m = Matcher::default();
        assert_eq!(feed_str(&mut m, &map, "1"), [run(Command::ZoomIn, None)]);
    }
}

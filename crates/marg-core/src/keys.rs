//! Key presses and their text notation for the keymap.
//!
//! Notation: a single character (`j`, `G`, `+`), a named key (`Space`,
//! `Esc`, `PageDown`), either with modifier prefixes (`Ctrl-o`,
//! `Shift-Space`, `Ctrl-Alt-x`). A sequence is written without separators
//! when it is plain characters (`gg`, `]a`), or as space-separated keys
//! (`g Ctrl-o`).

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Named {
    Space,
    Esc,
    Enter,
    Tab,
    Backspace,
    Delete,
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

const NAMES: &[(&str, Named)] = &[
    ("Space", Named::Space),
    ("Esc", Named::Esc),
    ("Enter", Named::Enter),
    ("Tab", Named::Tab),
    ("Backspace", Named::Backspace),
    ("Delete", Named::Delete),
    ("Up", Named::Up),
    ("Down", Named::Down),
    ("Left", Named::Left),
    ("Right", Named::Right),
    ("PageUp", Named::PageUp),
    ("PageDown", Named::PageDown),
    ("Home", Named::Home),
    ("End", Named::End),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum KeyCode {
    /// The logical character from the keyboard layout. With Ctrl or Alt,
    /// the lowercase letter of the key.
    Char(char),
    Named(Named),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct KeyPress {
    pub code: KeyCode,
    pub ctrl: bool,
    pub alt: bool,
    /// Only for named keys; for characters, shift is part of the character.
    pub shift: bool,
}

impl KeyPress {
    pub fn char(c: char) -> Self {
        Self {
            code: KeyCode::Char(c),
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    pub fn named(n: Named) -> Self {
        Self {
            code: KeyCode::Named(n),
            ctrl: false,
            alt: false,
            shift: false,
        }
    }

    pub fn with_ctrl(self) -> Self {
        Self { ctrl: true, ..self }
    }

    /// The digit of an unmodified digit key.
    pub fn digit(&self) -> Option<u32> {
        match self.code {
            KeyCode::Char(c) if !self.ctrl && !self.alt => c.to_digit(10),
            _ => None,
        }
    }

    /// Parse one key, e.g. `j`, `Ctrl-o`, `Shift-Space`.
    pub fn parse(s: &str) -> Option<Self> {
        let mut key = Self::char(' ');
        let mut rest = s;
        loop {
            if let Some(r) = rest.strip_prefix("Ctrl-").filter(|r| !r.is_empty()) {
                key.ctrl = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("Alt-").filter(|r| !r.is_empty()) {
                key.alt = true;
                rest = r;
            } else if let Some(r) = rest.strip_prefix("Shift-").filter(|r| !r.is_empty()) {
                key.shift = true;
                rest = r;
            } else {
                break;
            }
        }
        if let Some(&(_, n)) = NAMES.iter().find(|(name, _)| *name == rest) {
            key.code = KeyCode::Named(n);
            return Some(key);
        }
        let mut chars = rest.chars();
        let c = chars.next()?;
        if chars.next().is_some() || key.shift {
            return None;
        }
        // Ctrl/Alt with a letter: matched on the lowercase letter.
        key.code = KeyCode::Char(if key.ctrl || key.alt {
            c.to_ascii_lowercase()
        } else {
            c
        });
        Some(key)
    }
}

impl fmt::Display for KeyPress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.ctrl {
            f.write_str("Ctrl-")?;
        }
        if self.alt {
            f.write_str("Alt-")?;
        }
        if self.shift {
            f.write_str("Shift-")?;
        }
        match self.code {
            KeyCode::Char(c) => write!(f, "{c}"),
            KeyCode::Named(n) => {
                let name = NAMES.iter().find(|(_, k)| *k == n).map_or("?", |(s, _)| s);
                f.write_str(name)
            }
        }
    }
}

/// Parse a key sequence, e.g. `gg`, `Ctrl-o`, `]a`, `g Ctrl-o`.
pub fn parse_sequence(s: &str) -> Option<Vec<KeyPress>> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.contains(char::is_whitespace) {
        return s.split_whitespace().map(KeyPress::parse).collect();
    }
    if let Some(k) = KeyPress::parse(s) {
        return Some(vec![k]);
    }
    Some(s.chars().map(KeyPress::char).collect())
}

/// Display a sequence the way it is written in the config.
pub fn format_sequence(keys: &[KeyPress]) -> String {
    let plain = keys
        .iter()
        .all(|k| matches!(k.code, KeyCode::Char(_)) && !k.ctrl && !k.alt);
    let parts: Vec<String> = keys.iter().map(ToString::to_string).collect();
    parts.join(if plain { "" } else { " " })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_keys() {
        assert_eq!(KeyPress::parse("j"), Some(KeyPress::char('j')));
        assert_eq!(KeyPress::parse("G"), Some(KeyPress::char('G')));
        assert_eq!(
            KeyPress::parse("Ctrl-o"),
            Some(KeyPress::char('o').with_ctrl())
        );
        assert_eq!(
            KeyPress::parse("Ctrl-O"),
            Some(KeyPress::char('o').with_ctrl())
        );
        let shift_space = KeyPress {
            shift: true,
            ..KeyPress::named(Named::Space)
        };
        assert_eq!(KeyPress::parse("Shift-Space"), Some(shift_space));
        assert_eq!(KeyPress::parse("Shift-a"), None);
        assert_eq!(KeyPress::parse("-"), Some(KeyPress::char('-')));
        assert_eq!(
            KeyPress::parse("Ctrl--"),
            Some(KeyPress::char('-').with_ctrl())
        );
    }

    #[test]
    fn parses_sequences() {
        let g = KeyPress::char('g');
        assert_eq!(parse_sequence("gg"), Some(vec![g, g]));
        assert_eq!(
            parse_sequence("]a"),
            Some(vec![KeyPress::char(']'), KeyPress::char('a')])
        );
        assert_eq!(
            parse_sequence("g Ctrl-o"),
            Some(vec![g, KeyPress::char('o').with_ctrl()])
        );
        assert_eq!(
            parse_sequence("Space"),
            Some(vec![KeyPress::named(Named::Space)])
        );
        assert_eq!(parse_sequence("g Bogus-x"), None);
        assert_eq!(parse_sequence(""), None);
    }

    #[test]
    fn formats_round_trip() {
        for s in ["gg", "Ctrl-o", "Shift-Space", "g Ctrl-o", "]a", "PageDown"] {
            assert_eq!(format_sequence(&parse_sequence(s).unwrap()), s);
        }
    }
}

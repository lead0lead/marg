//! Translating egui input events into marg key presses.

use eframe::egui::{Event, Key, Modifiers};
use marg_core::keys::{KeyPress, Named};

fn named(key: Key) -> Option<Named> {
    Some(match key {
        Key::Space => Named::Space,
        Key::Escape => Named::Esc,
        Key::Enter => Named::Enter,
        Key::Tab => Named::Tab,
        Key::Backspace => Named::Backspace,
        Key::Delete => Named::Delete,
        Key::ArrowUp => Named::Up,
        Key::ArrowDown => Named::Down,
        Key::ArrowLeft => Named::Left,
        Key::ArrowRight => Named::Right,
        Key::PageUp => Named::PageUp,
        Key::PageDown => Named::PageDown,
        Key::Home => Named::Home,
        Key::End => Named::End,
        _ => return None,
    })
}

/// A key press from one egui event, if it is one.
///
/// Characters come from text events (the logical character of the active
/// layout), except with Ctrl or Alt, where the key's own symbol is used.
/// Space is taken from the key event so that `Shift-Space` works.
///
/// AltGr: on Windows it arrives as Ctrl+Alt, and egui-winit drops the text
/// event whenever Ctrl is held, so the key event's logical symbol (e.g. `[`
/// on a German layout) is used as a plain character. Elsewhere AltGr
/// produces a normal text event.
pub fn key_press(event: &Event) -> Option<KeyPress> {
    match event {
        Event::Text(t) => {
            let mut chars = t.chars();
            let c = chars.next()?;
            (chars.next().is_none() && c != ' ').then(|| KeyPress::char(c))
        }
        Event::Key {
            key,
            pressed: true,
            modifiers,
            ..
        } => {
            let ctrl = modifiers.ctrl || modifiers.mac_cmd;
            let alt = modifiers.alt;
            if let Some(n) = named(*key) {
                return Some(KeyPress {
                    ctrl,
                    alt,
                    shift: modifiers.shift,
                    ..KeyPress::named(n)
                });
            }
            let altgr = cfg!(windows) && ctrl && alt;
            if !ctrl && !alt {
                // A text event carries this key.
                return None;
            }
            let symbol = key.symbol_or_name();
            let mut chars = symbol.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            if altgr {
                return Some(KeyPress::char(c));
            }
            Some(KeyPress {
                ctrl,
                alt,
                ..KeyPress::char(c.to_ascii_lowercase())
            })
        }
        _ => None,
    }
}

/// Whether the modifiers ask for zoom on the mouse wheel.
pub fn wheel_zooms(m: &Modifiers) -> bool {
    m.command
}

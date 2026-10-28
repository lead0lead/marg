//! Moving and filtering in overlay lists (recent files; later the outline).
//!
//! A list opens for moving: `j`/`k`, `gg`/`G`, `Ctrl-d`/`Ctrl-u`, Enter
//! opens, `/` starts filtering, Esc clears the filter or closes the list.
//! While filtering, typing narrows the list (best match selected),
//! `Ctrl-n`/`Ctrl-p` and the arrow keys move, Enter opens, and Esc stops
//! typing but keeps the filter.

use crate::keys::{KeyCode, KeyPress, Named};
use crate::lineedit::LineEdit;

#[derive(Debug, Clone, Default)]
pub struct ListNav {
    filter: LineEdit,
    filtering: bool,
    /// Index into the filtered entries.
    selected: usize,
    /// Number of filtered entries.
    len: usize,
    /// A `g` was typed; a second one goes to the top.
    pending_g: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListAction {
    None,
    /// The filter text changed: refilter, then call `set_len`.
    FilterChanged,
    /// Open the selected entry.
    Open,
    Close,
}

impl ListNav {
    pub fn filter(&self) -> &str {
        self.filter.text()
    }

    /// Cursor position in the filter, in characters.
    pub fn cursor(&self) -> usize {
        self.filter.cursor()
    }

    pub fn filtering(&self) -> bool {
        self.filtering
    }

    /// The selected entry, if any.
    pub fn selected(&self) -> Option<usize> {
        (self.selected < self.len).then_some(self.selected)
    }

    /// The list now has `len` entries, best first; select the first.
    pub fn set_len(&mut self, len: usize) {
        self.len = len;
        self.selected = 0;
    }

    /// Select entry `i` (the last one if `i` is past the end).
    pub fn select(&mut self, i: usize) {
        self.selected = i.min(self.len.saturating_sub(1));
    }

    /// Typed text; only used while filtering.
    pub fn text(&mut self, t: &str) -> ListAction {
        if !self.filtering {
            return ListAction::None;
        }
        self.filter.insert(t);
        ListAction::FilterChanged
    }

    /// A key press. `half_page`: rows moved by `Ctrl-d`/`Ctrl-u`.
    pub fn key(&mut self, k: KeyPress, half_page: usize) -> ListAction {
        if self.filtering {
            self.filter_key(k)
        } else {
            self.move_key(k, half_page.max(1))
        }
    }

    fn filter_key(&mut self, k: KeyPress) -> ListAction {
        use Named::*;
        match (k.code, k.ctrl) {
            (KeyCode::Named(Esc), _) => self.filtering = false,
            (KeyCode::Named(Enter), _) => return self.open(),
            (KeyCode::Named(Backspace), _) if self.filter.is_empty() => self.filtering = false,
            (KeyCode::Named(Backspace), _) => {
                self.filter.backspace();
                return ListAction::FilterChanged;
            }
            (KeyCode::Named(Delete), _) => {
                self.filter.delete();
                return ListAction::FilterChanged;
            }
            (KeyCode::Named(Left), _) => self.filter.left(),
            (KeyCode::Named(Right), _) => self.filter.right(),
            (KeyCode::Named(Home), _) => self.filter.home(),
            (KeyCode::Named(End), _) => self.filter.end(),
            (KeyCode::Named(Down), _) | (KeyCode::Char('n'), true) => self.step(1),
            (KeyCode::Named(Up), _) | (KeyCode::Char('p'), true) => self.step(-1),
            _ => {}
        }
        ListAction::None
    }

    fn move_key(&mut self, k: KeyPress, half: usize) -> ListAction {
        use Named::*;
        let g = std::mem::take(&mut self.pending_g);
        let half = half as isize;
        match (k.code, k.ctrl) {
            (KeyCode::Char('g'), false) if g => self.selected = 0,
            (KeyCode::Char('g'), false) => self.pending_g = true,
            (KeyCode::Char('j'), false)
            | (KeyCode::Named(Down), _)
            | (KeyCode::Char('n'), true) => self.step(1),
            (KeyCode::Char('k'), false) | (KeyCode::Named(Up), _) | (KeyCode::Char('p'), true) => {
                self.step(-1)
            }
            (KeyCode::Char('d'), true) | (KeyCode::Named(PageDown), _) => self.step(half),
            (KeyCode::Char('u'), true) | (KeyCode::Named(PageUp), _) => self.step(-half),
            (KeyCode::Char('G'), false) | (KeyCode::Named(End), _) => {
                self.selected = self.len.saturating_sub(1)
            }
            (KeyCode::Named(Home), _) => self.selected = 0,
            (KeyCode::Char('/'), false) => self.filtering = true,
            (KeyCode::Named(Enter), _) => return self.open(),
            (KeyCode::Named(Esc), _) if self.filter.is_empty() => return ListAction::Close,
            (KeyCode::Named(Esc), _) => {
                self.filter.clear();
                return ListAction::FilterChanged;
            }
            _ => {}
        }
        ListAction::None
    }

    fn open(&mut self) -> ListAction {
        if self.selected().is_some() {
            self.filtering = false;
            ListAction::Open
        } else {
            ListAction::None
        }
    }

    /// Move the selection, stopping at the ends.
    fn step(&mut self, d: isize) {
        let last = self.len.saturating_sub(1) as isize;
        self.selected = (self.selected as isize + d).clamp(0, last) as usize;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(ch: char) -> KeyPress {
        KeyPress::char(ch)
    }

    fn n(k: Named) -> KeyPress {
        KeyPress::named(k)
    }

    fn list(len: usize) -> ListNav {
        let mut l = ListNav::default();
        l.set_len(len);
        l
    }

    #[test]
    fn moves_with_vim_keys_and_stops_at_ends() {
        let mut l = list(5);
        l.key(c('k'), 2);
        assert_eq!(l.selected(), Some(0));
        l.key(c('j'), 2);
        l.key(c('j'), 2);
        assert_eq!(l.selected(), Some(2));
        l.key(c('G'), 2);
        assert_eq!(l.selected(), Some(4));
        l.key(c('j'), 2);
        assert_eq!(l.selected(), Some(4));
        l.key(c('g'), 2);
        l.key(c('g'), 2);
        assert_eq!(l.selected(), Some(0));
        l.key(c('d').with_ctrl(), 2);
        assert_eq!(l.selected(), Some(2));
        l.key(c('u').with_ctrl(), 2);
        assert_eq!(l.selected(), Some(0));
        assert_eq!(l.filter(), "");
    }

    #[test]
    fn single_g_then_other_key_does_not_jump() {
        let mut l = list(5);
        l.key(c('G'), 2);
        l.key(c('g'), 2);
        l.key(c('k'), 2);
        l.key(c('g'), 2);
        assert_eq!(l.selected(), Some(3));
    }

    #[test]
    fn letters_only_filter_after_slash() {
        let mut l = list(5);
        assert_eq!(l.text("j"), ListAction::None);
        l.key(c('/'), 2);
        assert!(l.filtering());
        assert_eq!(l.text("jk"), ListAction::FilterChanged);
        assert_eq!(l.filter(), "jk");
        l.set_len(3);
        l.key(c('n').with_ctrl(), 2);
        assert_eq!(l.selected(), Some(1));
    }

    #[test]
    fn enter_while_filtering_opens() {
        let mut l = list(5);
        l.key(c('/'), 2);
        l.text("ka");
        l.set_len(1);
        assert_eq!(l.key(n(Named::Enter), 2), ListAction::Open);
        assert!(!l.filtering());
    }

    #[test]
    fn enter_with_no_match_does_nothing() {
        let mut l = list(5);
        l.key(c('/'), 2);
        l.text("zz");
        l.set_len(0);
        assert_eq!(l.selected(), None);
        assert_eq!(l.key(n(Named::Enter), 2), ListAction::None);
        assert!(l.filtering());
    }

    #[test]
    fn esc_keeps_filter_then_clears_then_closes() {
        let mut l = list(5);
        l.key(c('/'), 2);
        l.text("ka");
        l.set_len(3);
        assert_eq!(l.key(n(Named::Esc), 2), ListAction::None);
        assert!(!l.filtering());
        assert_eq!(l.filter(), "ka");
        l.key(c('j'), 2);
        assert_eq!(l.selected(), Some(1));
        assert_eq!(l.key(n(Named::Esc), 2), ListAction::FilterChanged);
        assert_eq!(l.filter(), "");
        assert_eq!(l.key(n(Named::Esc), 2), ListAction::Close);
    }

    #[test]
    fn backspace_on_empty_filter_stops_filtering() {
        let mut l = list(5);
        l.key(c('/'), 2);
        l.text("a");
        assert_eq!(l.key(n(Named::Backspace), 2), ListAction::FilterChanged);
        assert!(l.filtering());
        l.key(n(Named::Backspace), 2);
        assert!(!l.filtering());
    }

    #[test]
    fn slash_again_continues_the_filter() {
        let mut l = list(5);
        l.key(c('/'), 2);
        l.text("ka");
        l.key(n(Named::Esc), 2);
        l.key(c('/'), 2);
        l.text("h");
        assert_eq!(l.filter(), "kah");
    }
}

//! A one-line text editor for the command line, with history and
//! completion helpers.

#[derive(Debug, Clone, Default)]
pub struct LineEdit {
    text: String,
    /// Cursor position in characters.
    cursor: usize,
    history: Vec<String>,
    /// Index into `history` while browsing it; `None` = editing a new line.
    browsing: Option<usize>,
    /// The new line being edited, kept while browsing the history.
    draft: String,
}

impl LineEdit {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn byte_at(&self, char_idx: usize) -> usize {
        self.text
            .char_indices()
            .nth(char_idx)
            .map_or(self.text.len(), |(i, _)| i)
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    /// Replace the text and put the cursor at its end.
    pub fn set(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor = self.len();
    }

    /// Start a new empty line.
    pub fn clear(&mut self) {
        self.set("");
        self.browsing = None;
    }

    pub fn insert(&mut self, s: &str) {
        let at = self.byte_at(self.cursor);
        self.text.insert_str(at, s);
        self.cursor += s.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            let at = self.byte_at(self.cursor);
            self.text.remove(at);
        }
    }

    pub fn delete(&mut self) {
        if self.cursor < self.len() {
            let at = self.byte_at(self.cursor);
            self.text.remove(at);
        }
    }

    pub fn left(&mut self) {
        self.cursor = self.cursor.saturating_sub(1);
    }

    pub fn right(&mut self) {
        self.cursor = (self.cursor + 1).min(self.len());
    }

    pub fn home(&mut self) {
        self.cursor = 0;
    }

    pub fn end(&mut self) {
        self.cursor = self.len();
    }

    /// Finish the line: add it to the history (if new) and return it.
    pub fn submit(&mut self) -> String {
        let line = std::mem::take(&mut self.text);
        if !line.trim().is_empty() && self.history.last() != Some(&line) {
            self.history.push(line.clone());
        }
        self.clear();
        line
    }

    pub fn history_prev(&mut self) {
        let i = match self.browsing {
            None if self.history.is_empty() => return,
            None => {
                self.draft = self.text.clone();
                self.history.len() - 1
            }
            Some(0) => return,
            Some(i) => i - 1,
        };
        self.browsing = Some(i);
        let line = self.history[i].clone();
        self.set(&line);
    }

    pub fn history_next(&mut self) {
        match self.browsing {
            None => {}
            Some(i) if i + 1 < self.history.len() => {
                self.browsing = Some(i + 1);
                let line = self.history[i + 1].clone();
                self.set(&line);
            }
            Some(_) => {
                self.browsing = None;
                let draft = std::mem::take(&mut self.draft);
                self.set(&draft);
            }
        }
    }
}

/// The longest common prefix of all candidates.
pub fn common_prefix<S: AsRef<str>>(candidates: &[S]) -> String {
    let Some(first) = candidates.first() else {
        return String::new();
    };
    let mut prefix = first.as_ref();
    for c in &candidates[1..] {
        let c = c.as_ref();
        let n = prefix
            .char_indices()
            .zip(c.chars())
            .take_while(|((_, a), b)| a == b)
            .last()
            .map_or(0, |((i, a), _)| i + a.len_utf8());
        prefix = &prefix[..n];
    }
    prefix.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_at_cursor() {
        let mut l = LineEdit::default();
        l.insert("e paper.pdf");
        l.home();
        l.right();
        l.right();
        l.insert("ä/");
        assert_eq!(l.text(), "e ä/paper.pdf");
        l.backspace();
        l.backspace();
        assert_eq!(l.text(), "e paper.pdf");
        l.delete();
        assert_eq!(l.text(), "e aper.pdf");
        l.end();
        l.backspace();
        assert_eq!(l.text(), "e aper.pd");
    }

    #[test]
    fn history_browsing_keeps_draft() {
        let mut l = LineEdit::default();
        l.insert("q");
        assert_eq!(l.submit(), "q");
        l.insert("version");
        l.submit();
        l.insert("draft");
        l.history_prev();
        assert_eq!(l.text(), "version");
        l.history_prev();
        assert_eq!(l.text(), "q");
        l.history_prev();
        assert_eq!(l.text(), "q");
        l.history_next();
        l.history_next();
        assert_eq!(l.text(), "draft");
    }

    #[test]
    fn common_prefix_of_candidates() {
        assert_eq!(common_prefix(&["paper1.pdf", "paper2.pdf"]), "paper");
        assert_eq!(common_prefix(&["über.pdf", "übel.pdf"]), "übe");
        assert_eq!(common_prefix(&["a"]), "a");
        assert_eq!(common_prefix::<&str>(&[]), "");
        assert_eq!(common_prefix(&["abc", "xyz"]), "");
    }
}

//! Jump history (`Ctrl-o` / `Ctrl-i`), like vim's jump list.

use crate::view::DocPos;

#[derive(Debug, Clone)]
pub struct JumpList {
    back: Vec<DocPos>,
    forward: Vec<DocPos>,
    max: usize,
}

impl JumpList {
    pub fn new(max: usize) -> Self {
        Self {
            back: Vec::new(),
            forward: Vec::new(),
            max: max.max(1),
        }
    }

    /// Before a jump: remember `from`. A new jump drops the forward
    /// history.
    pub fn push(&mut self, from: DocPos) {
        self.forward.clear();
        if self.back.last() != Some(&from) {
            self.back.push(from);
        }
        if self.back.len() > self.max {
            self.back.remove(0);
        }
    }

    /// `Ctrl-o`: where to go from `current`, if anywhere.
    pub fn back(&mut self, current: DocPos) -> Option<DocPos> {
        let to = self.back.pop()?;
        self.forward.push(current);
        Some(to)
    }

    /// `Ctrl-i`: undo a `back`.
    pub fn forward(&mut self, current: DocPos) -> Option<DocPos> {
        let to = self.forward.pop()?;
        self.back.push(current);
        Some(to)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(page: usize) -> DocPos {
        DocPos {
            page,
            x: 0.0,
            y: 0.0,
        }
    }

    #[test]
    fn back_and_forward() {
        let mut j = JumpList::new(100);
        j.push(at(1)); // 1 → 5
        j.push(at(5)); // 5 → 9
        assert_eq!(j.back(at(9)), Some(at(5)));
        assert_eq!(j.back(at(5)), Some(at(1)));
        assert_eq!(j.back(at(1)), None);
        assert_eq!(j.forward(at(1)), Some(at(5)));
        assert_eq!(j.forward(at(5)), Some(at(9)));
        assert_eq!(j.forward(at(9)), None);
    }

    #[test]
    fn new_jump_drops_forward_history_and_limit_holds() {
        let mut j = JumpList::new(2);
        j.push(at(1));
        j.push(at(2));
        j.back(at(3));
        j.push(at(2)); // same as the last entry: kept once
        assert_eq!(j.forward(at(0)), None);
        j.push(at(7));
        j.push(at(8));
        assert_eq!(j.back(at(9)), Some(at(8)));
        assert_eq!(j.back(at(8)), Some(at(7)));
        assert_eq!(j.back(at(7)), None);
    }
}

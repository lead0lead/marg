//! A bounded undo/redo stack.

#[derive(Debug, Clone)]
pub struct UndoStack<T> {
    done: Vec<T>,
    undone: Vec<T>,
    max: usize,
}

impl<T> UndoStack<T> {
    /// Keeps at most `max` steps; the oldest are dropped.
    pub fn new(max: usize) -> Self {
        Self {
            done: Vec::new(),
            undone: Vec::new(),
            max: max.max(1),
        }
    }

    /// Record a new step. Steps undone before are dropped.
    pub fn push(&mut self, step: T) {
        self.undone.clear();
        self.done.push(step);
        if self.done.len() > self.max {
            self.done.remove(0);
        }
    }

    /// Take the last step to undo it; it can then be redone.
    pub fn undo(&mut self) -> Option<&mut T> {
        let step = self.done.pop()?;
        self.undone.push(step);
        self.undone.last_mut()
    }

    /// Take the last undone step to redo it.
    pub fn redo(&mut self) -> Option<&mut T> {
        let step = self.undone.pop()?;
        self.done.push(step);
        self.done.last_mut()
    }

    /// All steps, done and undone.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.done.iter_mut().chain(self.undone.iter_mut())
    }

    /// Forget the last step without undoing it (it cannot be redone).
    pub fn discard_last(&mut self) -> Option<T> {
        self.done.pop()
    }

    pub fn clear(&mut self) {
        self.done.clear();
        self.undone.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn undo_redo_and_new_step_drops_redo() {
        let mut u = UndoStack::new(10);
        u.push(1);
        u.push(2);
        assert_eq!(u.undo().copied(), Some(2));
        assert_eq!(u.undo().copied(), Some(1));
        assert_eq!(u.undo().copied(), None);
        assert_eq!(u.redo().copied(), Some(1));
        u.push(3);
        assert_eq!(u.redo().copied(), None);
        assert_eq!(u.undo().copied(), Some(3));
        assert_eq!(u.undo().copied(), Some(1));
    }

    #[test]
    fn discard_last_cannot_be_redone() {
        let mut u = UndoStack::new(10);
        u.push(1);
        u.push(2);
        assert_eq!(u.discard_last(), Some(2));
        assert_eq!(u.redo().copied(), None);
        assert_eq!(u.undo().copied(), Some(1));
    }

    #[test]
    fn bounded() {
        let mut u = UndoStack::new(2);
        for i in 0..5 {
            u.push(i);
        }
        assert_eq!(u.undo().copied(), Some(4));
        assert_eq!(u.undo().copied(), Some(3));
        assert_eq!(u.undo().copied(), None);
    }

    #[test]
    fn steps_can_be_updated() {
        let mut u = UndoStack::new(5);
        u.push(1);
        u.push(2);
        u.undo();
        for s in u.iter_mut() {
            *s *= 10;
        }
        assert_eq!(u.redo().copied(), Some(20));
    }
}

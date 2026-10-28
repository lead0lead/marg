//! Hint labels: short, prefix-free labels for targets on screen, and the
//! state of typing one.

/// `n` prefix-free labels over `alphabet`, as short as possible: as many
/// single characters as the count allows, then longer ones. Earlier
/// targets get the shorter labels.
pub fn labels(n: usize, alphabet: &[char]) -> Vec<String> {
    if n == 0 || alphabet.is_empty() {
        return Vec::new();
    }
    if alphabet.len() == 1 {
        return (1..=n).map(|i| alphabet[0].to_string().repeat(i)).collect();
    }
    // Breadth-first: replace the first (shortest) label by its extensions
    // until there are enough. Leftover labels are the longest ones.
    let mut out: std::collections::VecDeque<String> =
        alphabet.iter().map(|c| c.to_string()).collect();
    while out.len() < n {
        let first = out.pop_front().expect("not empty");
        out.extend(alphabet.iter().map(|c| format!("{first}{c}")));
    }
    out.into_iter().take(n).collect()
}

/// Typing a label: the typed prefix and the labels it can still reach.
#[derive(Debug, Clone, Default)]
pub struct HintInput {
    labels: Vec<String>,
    typed: String,
}

impl HintInput {
    pub fn new(labels: Vec<String>) -> Self {
        Self {
            labels,
            typed: String::new(),
        }
    }

    pub fn typed(&self) -> &str {
        &self.typed
    }

    pub fn labels(&self) -> &[String] {
        &self.labels
    }

    /// Whether label `i` still matches what was typed.
    pub fn matches(&self, i: usize) -> bool {
        self.labels[i].starts_with(&self.typed)
    }

    /// Type a character. Returns the picked target once a label is
    /// complete. A character that matches no label is ignored.
    pub fn push(&mut self, c: char) -> Option<usize> {
        let typed = format!("{}{c}", self.typed);
        if !self.labels.iter().any(|l| l.starts_with(&typed)) {
            return None;
        }
        self.typed = typed;
        self.labels.iter().position(|l| *l == self.typed)
    }

    /// Remove the last typed character. Returns false if nothing was typed.
    pub fn pop(&mut self) -> bool {
        self.typed.pop().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abc() -> Vec<char> {
        "asdfghjkl".chars().collect()
    }

    fn prefix_free(l: &[String]) -> bool {
        l.iter().enumerate().all(|(i, a)| {
            l.iter()
                .enumerate()
                .all(|(j, b)| i == j || !b.starts_with(a.as_str()))
        })
    }

    #[test]
    fn few_targets_get_single_characters() {
        assert_eq!(labels(3, &abc()), ["a", "s", "d"]);
        assert_eq!(labels(9, &abc()).iter().filter(|l| l.len() == 1).count(), 9);
        assert!(labels(0, &abc()).is_empty());
    }

    #[test]
    fn more_targets_stay_prefix_free_and_short() {
        for n in [10, 17, 50, 81, 82, 300] {
            let l = labels(n, &abc());
            assert_eq!(l.len(), n);
            assert!(prefix_free(&l), "{n}: {l:?}");
            let longest = l.iter().map(String::len).max().unwrap();
            let needed = (1..).find(|&k| 9usize.pow(k) >= n).unwrap() as usize;
            assert_eq!(longest, needed, "{n}");
            // Shorter labels come first.
            assert!(l.windows(2).all(|w| w[0].len() <= w[1].len()));
        }
        // 10 targets: 8 single, 2 double.
        let l = labels(10, &abc());
        assert_eq!(l.iter().filter(|l| l.len() == 1).count(), 8);
    }

    #[test]
    fn typing_picks_and_ignores_misses() {
        let mut h = HintInput::new(labels(10, &abc()));
        assert_eq!(h.push('x'), None);
        assert_eq!(h.typed(), "");
        assert_eq!(h.push('a'), None);
        assert_eq!(h.typed(), "a");
        assert!(h.matches(8) && !h.matches(0));
        assert!(h.pop());
        assert!(!h.pop());
        assert_eq!(h.push('s'), Some(0));
    }
}

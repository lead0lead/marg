//! A small subsequence fuzzy scorer for filterable lists.
//!
//! The query's characters must appear in order in the candidate. Matching
//! ignores case unless the query has an uppercase letter (smartcase).
//! Higher scores are better: consecutive matches, matches at word starts
//! and an early first match score higher.

fn is_boundary(prev: Option<char>, c: char) -> bool {
    match prev {
        None => true,
        Some(p) => !p.is_alphanumeric() || (p.is_lowercase() && c.is_uppercase()),
    }
}

/// Score `candidate` against `query`, or `None` if it does not match.
/// An empty query matches everything with score 0.
pub fn score(query: &str, candidate: &str) -> Option<i64> {
    if query.is_empty() {
        return Some(0);
    }
    let smart = query.chars().any(char::is_uppercase);
    let fold = |c: char| {
        if smart {
            c
        } else {
            c.to_lowercase().next().unwrap_or(c)
        }
    };
    let mut q = query.chars().map(fold).peekable();
    let mut total = 0i64;
    let mut prev: Option<char> = None;
    let mut last_match: Option<usize> = None;
    let mut first_match: Option<usize> = None;
    for (i, c) in candidate.chars().enumerate() {
        let Some(&want) = q.peek() else { break };
        if fold(c) == want {
            q.next();
            let mut s = 1;
            if last_match.is_some_and(|l| l + 1 == i) {
                s += 5;
            }
            if is_boundary(prev, c) {
                s += 3;
            }
            total += s;
            first_match.get_or_insert(i);
            last_match = Some(i);
        }
        prev = Some(c);
    }
    if q.peek().is_some() {
        return None;
    }
    // Earlier first matches rank higher.
    Some(total * 100 - first_match.unwrap_or(0) as i64)
}

/// Indices of `items` matching `query`, best first; ties keep their order.
pub fn filter<S: AsRef<str>>(query: &str, items: &[S]) -> Vec<usize> {
    let mut hits: Vec<(i64, usize)> = items
        .iter()
        .enumerate()
        .filter_map(|(i, s)| score(query, s.as_ref()).map(|sc| (sc, i)))
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    hits.into_iter().map(|(_, i)| i).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subsequence_must_match_in_order() {
        assert!(score("abc", "a_b_c").is_some());
        assert!(score("acb", "abc").is_none());
        assert!(score("x", "").is_none());
        assert_eq!(score("", "anything"), Some(0));
    }

    #[test]
    fn smartcase() {
        assert!(score("pdf", "Paper.PDF").is_some());
        assert!(score("PDF", "paper.pdf").is_none());
        assert!(score("PDF", "paper.PDF").is_some());
    }

    #[test]
    fn prefers_consecutive_and_word_starts() {
        let items = ["grudin-2008.pdf", "group-dynamics-in-research.pdf"];
        assert_eq!(filter("gru", &items), [0, 1]);
        let items = ["d-y-n.pdf", "dynamics.pdf"];
        assert_eq!(filter("dyn", &items), [1, 0]);
    }

    #[test]
    fn filter_keeps_order_on_ties() {
        let items = ["a.pdf", "b.pdf", "c.pdf"];
        assert_eq!(filter("", &items), [0, 1, 2]);
        assert_eq!(filter("pdf", &items), [0, 1, 2]);
    }
}

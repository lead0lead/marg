//! The help overlay (`?`) and the help page (`:help`), generated from the
//! live keymaps and the command registry, so remaps show up.

use crate::command::{Command, REGISTRY};
use crate::keymap::Keymap;

#[derive(Debug, Clone, PartialEq)]
pub enum HelpLine {
    Heading(String),
    /// Keys (several joined with `, `) and what they do.
    Binding {
        keys: String,
        text: String,
    },
    /// A line of explanation.
    Text(String),
}

/// Topics, in the order shown.
pub const TOPICS: [&str; 4] = ["Moving", "Selecting", "Annotating", "Files"];

/// The topic of a command, by its name.
pub fn topic(c: &Command) -> &'static str {
    let name = c.name();
    let starts = |p: &[&str]| p.iter().any(|p| name.starts_with(p));
    if starts(&[
        "hint.caret",
        "hint.select",
        "caret.",
        "visual.",
        "selection.",
    ]) {
        "Selecting"
    } else if starts(&["annotate.", "annot.", "hint.annot", "undo", "redo"]) {
        "Annotating"
    } else if starts(&["file.", "quit", "cmdline", "version", "debug.", "help"]) {
        "Files"
    } else {
        "Moving"
    }
}

/// The help text of a command.
pub fn help_text(c: &Command) -> &'static str {
    REGISTRY
        .iter()
        .find(|(_, r, _)| r == c)
        .map_or("", |(_, _, h)| h)
}

/// One line per command bound in `map` and not bound the same way in
/// `base` (so a mode shows only what it adds), grouped by topic, in
/// registry order; a command's keys are joined.
pub fn bindings(map: &Keymap, base: Option<&Keymap>) -> Vec<(&'static str, HelpLine)> {
    let inherited = |keys: &str, c: &Command| {
        base.is_some_and(|b| b.iter().any(|(k, bc)| k == keys && bc == c))
    };
    let mut out = Vec::new();
    for topic_name in TOPICS {
        for (_, cmd, text) in REGISTRY {
            if topic(cmd) != topic_name {
                continue;
            }
            let keys: Vec<String> = map
                .iter()
                .filter(|(k, c)| *c == cmd && !inherited(k, c))
                .map(|(k, _)| k)
                .collect();
            if !keys.is_empty() {
                out.push((
                    topic_name,
                    HelpLine::Binding {
                        keys: keys.join(", "),
                        text: (*text).to_string(),
                    },
                ));
            }
        }
    }
    out
}

/// `bindings` as lines under a heading per topic, headings prefixed
/// with `title` (e.g. "Normal — ").
pub fn section(title: &str, map: &Keymap, base: Option<&Keymap>) -> Vec<HelpLine> {
    let mut out = Vec::new();
    let mut last = "";
    for (t, line) in bindings(map, base) {
        if t != last {
            let name = if title.is_empty() {
                t.to_string()
            } else {
                t.to_lowercase()
            };
            out.push(HelpLine::Heading(format!("{title}{name}")));
            last = t;
        }
        out.push(line);
    }
    out
}

fn fixed(heading: &str, rows: &[(&str, &str)]) -> Vec<HelpLine> {
    let mut out = vec![HelpLine::Heading(heading.into())];
    out.extend(rows.iter().map(|(k, t)| HelpLine::Binding {
        keys: (*k).into(),
        text: (*t).into(),
    }));
    out
}

/// The keymaps of the modes, for the help page.
pub struct Maps<'a> {
    pub normal: &'a Keymap,
    pub caret: &'a Keymap,
    pub visual: &'a Keymap,
    pub annot: &'a Keymap,
}

/// The whole help page. `categories`: the category names, in key order.
pub fn page(maps: &Maps, categories: &[String]) -> Vec<HelpLine> {
    let mut out = Vec::new();
    out.extend(section("Normal — ", maps.normal, None));
    out.extend(section("Caret — ", maps.caret, Some(maps.normal)));
    out.extend(section("Visual — ", maps.visual, Some(maps.caret)));
    out.extend(section(
        "Selected annotation — ",
        maps.annot,
        Some(maps.normal),
    ));
    out.extend(fixed(
        "Hints",
        &[
            ("label", "pick that target"),
            ("Backspace", "remove the last typed character"),
            ("Esc", "cancel"),
        ],
    ));
    out.push(HelpLine::Text(
        "Text hints come in two stages: every visible line gets a label at its left, then \
         each word of the picked line gets one. f puts the caret on the word, F selects from \
         one word to another."
            .into(),
    ));
    out.extend(fixed(
        "Box mode",
        &[
            ("label label", "the cells of two corners"),
            ("h j k l", "move the bottom-right corner (Ctrl: finely)"),
            ("H J K L", "move the top-left corner (Ctrl-Shift: finely)"),
            ("s", "snap to the image under the box"),
            ("1-5", "category"),
            ("Enter / Esc", "save / cancel"),
        ],
    ));
    out.extend(fixed(
        "Comment editor",
        &[
            ("Enter", "new line"),
            ("Ctrl-Enter", "save"),
            ("Esc", "cancel (twice when the text changed)"),
        ],
    ));
    out.extend(fixed(
        "Lists (recent files, outline, annotations, help)",
        &[
            ("j / k, Ctrl-n / Ctrl-p", "move"),
            ("gg / G", "first / last"),
            ("Ctrl-d / Ctrl-u", "half a page"),
            ("/", "filter; Enter opens the best match"),
            ("Enter", "open"),
            ("Esc", "clear the filter, then close"),
            ("annotations: 1-5, 0", "one category, all"),
            ("annotations: e, x", "edit the comment, delete"),
        ],
    ));
    out.extend(fixed(
        "Search prompt",
        &[
            (
                "text",
                "search as you type (case matters only with capitals)",
            ),
            (
                "Enter",
                "keep the search; n / N go to the next / previous hit",
            ),
            (
                "Enter on an empty prompt",
                "search for the last query again",
            ),
            ("Esc", "back to where the search started"),
            ("Esc in Normal mode", "hide the hits (n searches again)"),
        ],
    ));
    out.extend(fixed(
        "Commands (:)",
        &[
            (":w, :w!", "save; save over changes made by another program"),
            (":q, :q!", "save and quit; quit without saving"),
            (
                ":e path, :e!",
                "open a file (Tab completes); reload from disk",
            ),
            (":close, :bd", "close the file, show the recent files"),
            (":layout continuous|single|spread", "set the layout"),
            (
                ":export md [path]",
                "write the annotations as Markdown (default: <file>.notes.md)",
            ),
            (":42", "go to page 42"),
            (":help", "this page"),
            (":version, :debug", "version; debug overlay"),
            (":<command name>", "run any command below, e.g. :zoom.in"),
        ],
    ));
    out.push(HelpLine::Heading("Categories".into()));
    for (i, name) in categories.iter().enumerate().take(9) {
        out.push(HelpLine::Binding {
            keys: (i + 1).to_string(),
            text: name.clone(),
        });
    }
    out.push(HelpLine::Text(
        "The category name is written into the annotation's subject; other programs' \
         annotations get a category by subject, else by the nearest color."
            .into(),
    ));
    out.push(HelpLine::Heading("Command names".into()));
    for (name, _, text) in REGISTRY {
        out.push(HelpLine::Binding {
            keys: (*name).into(),
            text: (*text).into(),
        });
    }
    out
}

/// The lines matching a filter, with the heading of each match: every
/// word of the query must appear in the keys or text (ignoring case;
/// fuzzy matching would match most long lines). An empty filter keeps
/// everything.
pub fn filter(lines: &[HelpLine], query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return (0..lines.len()).collect();
    }
    let mut out = Vec::new();
    let mut heading = None;
    for (i, l) in lines.iter().enumerate() {
        let hay = match l {
            HelpLine::Heading(_) => {
                heading = Some(i);
                continue;
            }
            HelpLine::Binding { keys, text } => format!("{keys} {text}"),
            HelpLine::Text(t) => t.clone(),
        };
        let hay = hay.to_lowercase();
        if words.iter().all(|w| hay.contains(w.as_str())) {
            if let Some(h) = heading.take() {
                out.push(h);
            }
            out.push(i);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::KeysConfig;
    use crate::keymap::KeyMode;
    use std::collections::BTreeMap;

    fn maps(keys: &KeysConfig) -> [Keymap; 4] {
        [
            KeyMode::Normal,
            KeyMode::Caret,
            KeyMode::Visual,
            KeyMode::Annot,
        ]
        .map(|m| Keymap::build(m, keys).0)
    }

    fn has(lines: &[HelpLine], keys: &str, text: &str) -> bool {
        lines.iter().any(|l| {
            *l == HelpLine::Binding {
                keys: keys.into(),
                text: text.into(),
            }
        })
    }

    #[test]
    fn every_command_has_a_topic_and_help() {
        for (_, c, text) in REGISTRY {
            assert!(TOPICS.contains(&topic(c)));
            assert!(!text.is_empty());
            assert_eq!(help_text(c), *text);
        }
    }

    #[test]
    fn page_shows_remaps_and_mode_additions_only() {
        let keys = KeysConfig {
            normal: BTreeMap::from([("Ctrl-n".to_string(), "page.next".to_string())]),
            ..KeysConfig::default()
        };
        let [n, c, v, a] = maps(&keys);
        let maps = Maps {
            normal: &n,
            caret: &c,
            visual: &v,
            annot: &a,
        };
        let page = page(&maps, &["Key point".into()]);
        assert!(has(&page, "J, PageDown, Ctrl-n", "next page"));
        assert!(page.contains(&HelpLine::Heading("Normal — moving".into())));
        // Caret mode lists its own keys, not the Normal ones again.
        let caret = section("", &c, Some(&n));
        assert!(has(&caret, "w", "next word"));
        assert!(!has(&caret, "J, PageDown, Ctrl-n", "next page"));
        assert!(has(&page, "1", "Key point"));
    }

    #[test]
    fn filter_keeps_headings_of_matches() {
        let lines = vec![
            HelpLine::Heading("A".into()),
            HelpLine::Binding {
                keys: "j".into(),
                text: "scroll down".into(),
            },
            HelpLine::Heading("B".into()),
            HelpLine::Binding {
                keys: "/".into(),
                text: "search".into(),
            },
        ];
        assert_eq!(filter(&lines, "search"), [2, 3]);
        assert_eq!(filter(&lines, "Down J"), [0, 1]);
        // Not a subsequence match: "sd" is in neither line.
        assert!(filter(&lines, "sd").is_empty());
        assert_eq!(filter(&lines, "").len(), 4);
        assert!(filter(&lines, "zzz").is_empty());
    }
}

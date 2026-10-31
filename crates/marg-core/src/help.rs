//! The help overlay (`?`) and the help page (`:help`), generated from the
//! live keymaps and the command registry, so remaps show up.

use crate::command::{Command, REGISTRY};
use crate::config::{Config, KeysConfig};
use crate::keymap::{KeyMode, Keymap};

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
        "hint.visual",
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
            // Bindings to named search engines are not in the registry.
            if *cmd == Command::SelectionSearch(None) {
                for (k, c) in map.iter() {
                    if let Command::SelectionSearch(Some(name)) = c
                        && !inherited(&k, c)
                    {
                        out.push((
                            topic_name,
                            HelpLine::Binding {
                                keys: k,
                                text: format!("search the web with {name}"),
                            },
                        ));
                    }
                }
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
            ("recent files: :", "command line, e.g. :e path"),
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
            (":help", "the help page"),
            (
                ":config",
                "write a commented default config, if there is none",
            ),
            (
                ":set name=value, :set name",
                "change a setting for this session (e.g. :set zoom_step=1.2); show it",
            ),
            (
                ":unlock",
                "annotate this file although its permissions forbid it",
            ),
            (":version, :debug", "version; debug overlay"),
            (":<command name>", "run any command by name, e.g. :zoom.in"),
            ("Ctrl-V", "paste (also in search, password and filters)"),
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

/// The cheat sheet (§2.12): the help page of the default config, without
/// the list of command names, as Markdown with headings of `level`.
pub fn cheatsheet(level: usize) -> String {
    let keys = KeysConfig::default();
    let [normal, caret, visual, annot] = [
        KeyMode::Normal,
        KeyMode::Caret,
        KeyMode::Visual,
        KeyMode::Annot,
    ]
    .map(|m| Keymap::build(m, &keys).0);
    let maps = Maps {
        normal: &normal,
        caret: &caret,
        visual: &visual,
        annot: &annot,
    };
    let names: Vec<String> = Config::default()
        .categories
        .into_iter()
        .map(|c| c.name)
        .collect();
    let hashes = "#".repeat(level);
    let cell = |s: &str| {
        s.replace('|', "\\|")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    let mut out = String::new();
    let mut in_table = false;
    for line in merge_digits(page(&maps, &names)) {
        match line {
            HelpLine::Heading(h) if h == "Command names" => break,
            HelpLine::Heading(h) => {
                out.push_str(&format!("\n{hashes} {h}\n\n| Keys | |\n|---|---|\n"));
                in_table = true;
            }
            HelpLine::Binding { keys, text } => {
                out.push_str(&format!("| `{}` | {} |\n", cell(&keys), cell(&text)));
            }
            HelpLine::Text(t) => {
                if in_table {
                    out.push('\n');
                    in_table = false;
                }
                out.push_str(&format!("{t}\n"));
            }
        }
    }
    out
}

/// Consecutive bindings `1`…`n` whose texts end in the same digit, such
/// as "highlight, category 1", as one line ("1-5", "highlight, category
/// 1-5").
fn merge_digits(lines: Vec<HelpLine>) -> Vec<HelpLine> {
    let mut out: Vec<HelpLine> = Vec::new();
    let mut run: Vec<(String, String)> = Vec::new();
    let flush = |run: &mut Vec<(String, String)>, out: &mut Vec<HelpLine>| {
        if run.len() > 1 {
            let last = &run[run.len() - 1].0;
            let text = run[0].1.trim_end_matches('1');
            out.push(HelpLine::Binding {
                keys: format!("1-{last}"),
                text: format!("{text}1-{last}"),
            });
        } else {
            out.extend(
                run.drain(..)
                    .map(|(keys, text)| HelpLine::Binding { keys, text }),
            );
        }
        run.clear();
    };
    for line in lines {
        if let HelpLine::Binding { keys, text } = &line {
            let next = (run.len() + 1).to_string();
            let same = run.first().is_none_or(|(_, t)| {
                t.trim_end_matches('1') == text.trim_end_matches(next.as_str())
            });
            if *keys == next && text.ends_with(next.as_str()) && same {
                run.push((keys.clone(), text.clone()));
                continue;
            }
        }
        flush(&mut run, &mut out);
        // The line may start a new run.
        if let HelpLine::Binding { keys, text } = &line
            && keys == "1"
            && text.ends_with('1')
        {
            run.push((keys.clone(), text.clone()));
            continue;
        }
        out.push(line);
    }
    flush(&mut run, &mut out);
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
    use std::collections::BTreeMap;
    use std::path::Path;

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
    fn named_search_engines_are_listed() {
        let keys = KeysConfig {
            visual: BTreeMap::from([("gw".to_string(), "selection.search(wiki)".to_string())]),
            ..KeysConfig::default()
        };
        let [_, c, v, _] = maps(&keys);
        let visual = section("", &v, Some(&c));
        assert!(has(&visual, "gs", "search the web for the text"));
        assert!(has(&visual, "gw", "search the web with wiki"));
    }

    const SHEET_START: &str = "<!-- cheat sheet: generated, MARG_BLESS=1 cargo test updates it -->";
    const SHEET_END: &str = "<!-- end of cheat sheet -->";

    /// `docs/cheatsheet.md` and the README's cheat sheet are the defaults'
    /// (§2.12). `MARG_BLESS=1 cargo test -p marg-core cheat` rewrites
    /// them.
    #[test]
    fn committed_cheat_sheets_match_the_defaults() {
        // Git may check text out with CRLF line endings (Windows).
        let read = |p: &Path| std::fs::read_to_string(p).map(|t| t.replace("\r\n", "\n"));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let doc_path = root.join("docs/cheatsheet.md");
        let doc = format!(
            "# marg cheat sheet\n\n{SHEET_START}\n\nThe default keys. `?` shows the keys of the \
             current mode and `:help` everything, with your own bindings.\n{}{SHEET_END}\n",
            cheatsheet(2)
        );
        let readme_path = root.join("README.md");
        let readme = read(&readme_path).unwrap();
        let (Some(start), Some(end)) = (readme.find(SHEET_START), readme.find(SHEET_END)) else {
            panic!("README.md has no cheat sheet markers");
        };
        let new_readme = format!(
            "{}{SHEET_START}\n{}{}",
            &readme[..start],
            cheatsheet(3),
            &readme[end..]
        );
        if std::env::var_os("MARG_BLESS").is_some() {
            std::fs::write(&doc_path, &doc).unwrap();
            std::fs::write(&readme_path, &new_readme).unwrap();
            return;
        }
        let hint = "run MARG_BLESS=1 cargo test -p marg-core cheat to update";
        assert_eq!(read(&doc_path).ok(), Some(doc), "{hint}");
        assert_eq!(readme, new_readme, "{hint}");
    }

    #[test]
    fn cheat_sheet_tables() {
        let s = cheatsheet(2);
        assert!(s.contains("\n## Normal — moving\n\n| Keys | |\n|---|---|\n"));
        assert!(s.contains("| `f` | hint: place caret |"));
        assert!(
            s.contains("`:layout continuous\\|single\\|spread`"),
            "pipes escaped"
        );
        assert!(s.contains("| `1` | Key point |"), "names are not merged");
        assert!(s.contains("| `1-5` | highlight, category 1-5 |"));
        assert!(s.contains("| `1-5` | recolor: category 1-5 |"));
        assert!(s.contains("&lt;file&gt;.notes.md"));
        assert!(!s.contains("Command names"));
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

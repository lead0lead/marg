//! Web search for the selected text (`selection.search`): the search URL
//! from an engine's template, and checks of the `[search]` config.

use crate::command::Command;
use crate::config::Config;
use crate::keymap::{KeyMode, Keymap};

/// The search engine `selection.search` uses without a name.
pub const DEFAULT: &str = "default";

/// The query for `text`: whitespace collapsed to single spaces, cut at a
/// word boundary after `max_chars` characters. `true` if it was cut.
pub fn query(text: &str, max_chars: usize) -> (String, bool) {
    let mut out = String::new();
    let mut cut = false;
    for word in text.split_whitespace() {
        let sep = usize::from(!out.is_empty());
        if out.chars().count() + sep + word.chars().count() > max_chars {
            cut = true;
            // A single word longer than the limit is cut inside.
            if out.is_empty() {
                out = word.chars().take(max_chars).collect();
            }
            break;
        }
        if sep == 1 {
            out.push(' ');
        }
        out.push_str(word);
    }
    (out, cut)
}

/// `template` with each `%s` replaced by the percent-encoded `query`.
pub fn url(template: &str, query: &str) -> String {
    template.replace("%s", &encode(query))
}

/// Percent-encode everything but the unreserved characters of RFC 3986,
/// byte by byte in UTF-8; a space becomes `%20`.
fn encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Config problems, for the status bar: engines without `%s`, and Visual
/// bindings to engines that do not exist.
pub fn check(config: &Config) -> Vec<String> {
    let engines = &config.search.engines;
    let mut out: Vec<String> = engines
        .iter()
        .filter(|(_, t)| !t.contains("%s"))
        .map(|(name, _)| format!("search.engines: {name} has no %s for the text"))
        .collect();
    let (map, _) = Keymap::build(KeyMode::Visual, &config.keys);
    for (keys, c) in map.iter() {
        if let Command::SelectionSearch(Some(name)) = c
            && !engines.contains_key(name)
        {
            out.push(format!(
                "keys: \"{keys}\" searches with {name}, which is not in [search.engines]"
            ));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_collapses_whitespace_and_cuts_at_words() {
        assert_eq!(query("  a\n b\tc ", 100), ("a b c".into(), false));
        assert_eq!(query("one two three", 7), ("one two".into(), true));
        assert_eq!(query("one two three", 8), ("one two".into(), true));
        assert_eq!(query("one two", 7), ("one two".into(), false));
        assert_eq!(query("überlang", 4), ("über".into(), true));
        assert_eq!(query("", 10), (String::new(), false));
    }

    #[test]
    fn url_encodes_the_query() {
        assert_eq!(
            url("https://duckduckgo.com/?q=%s", "a&b c/ü?"),
            "https://duckduckgo.com/?q=a%26b%20c%2F%C3%BC%3F"
        );
        assert_eq!(url("x/%s/%s", "a.b~"), "x/a.b~/a.b~");
    }

    #[test]
    fn engines_add_to_the_default() {
        let c = Config::from_toml("[search.engines]\nwiki = \"https://w/?s=%s\"\n").unwrap();
        assert_eq!(
            c.search.engines.get(DEFAULT).map(String::as_str),
            Some(crate::config::DEFAULT_ENGINE)
        );
        assert!(c.search.engines.contains_key("wiki"));
        let c = Config::from_toml("[search.engines]\ndefault = \"https://g/?q=%s\"\n").unwrap();
        assert_eq!(c.search.engines[DEFAULT], "https://g/?q=%s");
        assert!(crate::config::unknown_settings("[search.engines]\nwiki = \"x%s\"\n").is_empty());
    }

    #[test]
    fn check_reports_missing_placeholder_and_unknown_engines() {
        assert!(check(&Config::default()).is_empty());
        let c = Config::from_toml(
            "[search.engines]\nwiki = \"https://w/\"\n\
             [keys.visual]\ngw = \"selection.search(wiki)\"\ngx = \"selection.search(nope)\"\n",
        )
        .unwrap();
        assert_eq!(
            check(&c),
            [
                "search.engines: wiki has no %s for the text",
                "keys: \"gx\" searches with nope, which is not in [search.engines]",
            ]
        );
    }

    #[test]
    fn command_names() {
        assert_eq!(
            Command::from_name("selection.search"),
            Some(Command::SelectionSearch(None))
        );
        assert_eq!(
            Command::from_name("selection.search( wiki )"),
            Some(Command::SelectionSearch(Some("wiki".into())))
        );
        assert_eq!(Command::from_name("selection.search()"), None);
    }
}

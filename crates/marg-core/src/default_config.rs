//! The commented default config that `:config` writes (§8).

use crate::command::REGISTRY;
use crate::keymap::{DEFAULT_ANNOT, DEFAULT_CARET, DEFAULT_NORMAL, DEFAULT_VISUAL, UNBIND};

/// Every setting, commented out with its default value, then the default
/// key bindings of each mode, also commented out.
pub fn text() -> String {
    let mut out = include_str!("default_config.toml").to_string();
    out.push_str(&format!(
        "\n# Key bindings: \"key\" = \"command\". Each table adds to the defaults \
         below;\n# \"{UNBIND}\" removes one. Keys: one character (j, G), a named key \
         (Space,\n# PageDown, Esc, Enter), with modifiers (Ctrl-o, Shift-Space), \
         or sequences\n# (gg, ]a). :help lists every command.\n"
    ));
    let tables = [
        ("normal", "Normal mode", DEFAULT_NORMAL),
        ("caret", "Caret mode, on top of normal", DEFAULT_CARET),
        ("visual", "Visual mode, on top of caret", DEFAULT_VISUAL),
        (
            "annot",
            "While an annotation is selected, on top of normal",
            DEFAULT_ANNOT,
        ),
    ];
    for (name, about, bindings) in tables {
        out.push_str(&format!("\n# {about}.\n#[keys.{name}]\n"));
        for (key, command) in bindings {
            let help = REGISTRY
                .iter()
                .find(|(n, _, _)| n == command)
                .map_or("", |(_, _, h)| h);
            let line = format!("#{} = {}", quoted(key), quoted(command));
            out.push_str(&format!("{line:<44} # {help}\n"));
        }
    }
    out
}

/// `s` as a TOML basic string.
fn quoted(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::config::{Config, KeysConfig};

    /// The config with every setting uncommented: lines starting with `#`
    /// directly followed by a key or a table header.
    fn uncommented() -> String {
        text()
            .lines()
            .map(|l| match l.strip_prefix('#') {
                Some(rest) if rest.starts_with(|c: char| !c.is_whitespace() && c != '#') => rest,
                _ => l,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Every key path in `t`, such as `view.zoom_step` (arrays of tables
    /// by their first element).
    fn paths(t: &toml::Table, prefix: &str, out: &mut Vec<String>) {
        for (k, v) in t {
            let p = format!("{prefix}{k}");
            match v {
                toml::Value::Table(t) => paths(t, &format!("{p}."), out),
                toml::Value::Array(a) => match a.first() {
                    Some(toml::Value::Table(t)) => paths(t, &format!("{p}[]."), out),
                    _ => out.push(p),
                },
                _ => out.push(p),
            }
        }
    }

    #[test]
    fn as_written_it_is_the_default() {
        assert_eq!(Config::from_toml(&text()).unwrap(), Config::default());
    }

    #[test]
    fn uncommented_it_sets_every_default_and_nothing_else() {
        let text = uncommented();
        let table = |m: &[(&str, &str)]| -> BTreeMap<String, String> {
            m.iter()
                .map(|(k, c)| (k.to_string(), c.to_string()))
                .collect()
        };
        let expected = Config {
            keys: KeysConfig {
                normal: table(DEFAULT_NORMAL),
                caret: table(DEFAULT_CARET),
                visual: table(DEFAULT_VISUAL),
                annot: table(DEFAULT_ANNOT),
            },
            ..Config::default()
        };
        assert_eq!(Config::from_toml(&text).unwrap(), expected);
        // Same keys, so every setting is documented and none is misspelt
        // (unknown keys would be ignored by the parse above).
        let mut written = Vec::new();
        paths(&toml::from_str(&text).unwrap(), "", &mut written);
        let mut known = Vec::new();
        paths(&toml::Table::try_from(&expected).unwrap(), "", &mut known);
        written.sort();
        known.sort();
        assert_eq!(written, known);
    }

    #[test]
    fn every_bound_command_has_help() {
        for line in text().lines().filter(|l| l.starts_with("#\"")) {
            assert!(!line.ends_with("# "), "no help text: {line}");
        }
    }
}

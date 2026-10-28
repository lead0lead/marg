//! Named commands. Keys, `:` commands and the mouse all run these.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
    HalfPageDown,
    HalfPageUp,
    PageNext,
    PagePrev,
    /// With a count: go to that page.
    PageFirst,
    /// With a count: go to that page.
    PageLast,
    /// Go to a 1-based page number.
    PageGoto(usize),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitWidth,
    FitPage,
    /// Open the `:` command line.
    CommandLine,
    Quit,
    QuitForce,
    Open(String),
    Version,
    DebugToggle,
}

/// Commands that keys can be bound to, with their name and help text.
pub const REGISTRY: &[(&str, Command, &str)] = &[
    ("scroll.down", Command::ScrollDown, "scroll down"),
    ("scroll.up", Command::ScrollUp, "scroll up"),
    ("scroll.left", Command::ScrollLeft, "scroll left"),
    ("scroll.right", Command::ScrollRight, "scroll right"),
    ("scroll.half_down", Command::HalfPageDown, "half page down"),
    ("scroll.half_up", Command::HalfPageUp, "half page up"),
    ("page.next", Command::PageNext, "next page"),
    ("page.prev", Command::PagePrev, "previous page"),
    ("page.first", Command::PageFirst, "first page (or page n)"),
    ("page.last", Command::PageLast, "last page (or page n)"),
    ("zoom.in", Command::ZoomIn, "zoom in"),
    ("zoom.out", Command::ZoomOut, "zoom out"),
    ("zoom.reset", Command::ZoomReset, "zoom 100%"),
    ("zoom.fit_width", Command::FitWidth, "fit width"),
    ("zoom.fit_page", Command::FitPage, "fit page"),
    ("cmdline", Command::CommandLine, "command line"),
    ("quit", Command::Quit, "quit"),
    ("quit.force", Command::QuitForce, "quit without saving"),
    ("version", Command::Version, "show version"),
    ("debug.toggle", Command::DebugToggle, "debug overlay"),
];

impl Command {
    /// Look up a bindable command by name.
    pub fn from_name(name: &str) -> Option<Self> {
        REGISTRY
            .iter()
            .find(|(n, _, _)| *n == name)
            .map(|(_, c, _)| c.clone())
    }

    pub fn name(&self) -> &'static str {
        match self {
            Command::PageGoto(_) => "page.goto",
            Command::Open(_) => "file.open",
            c => REGISTRY
                .iter()
                .find(|(_, r, _)| r == c)
                .map_or("?", |(n, _, _)| n),
        }
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Parse a `:` command line (without the colon).
///
/// `q`, `q!`, `e <path>`, `version`, `debug`, a page number, or any
/// registered command name (`zoom.in`).
pub fn parse_ex(line: &str) -> Result<Command, String> {
    let line = line.trim();
    let (word, rest) = match line.split_once(char::is_whitespace) {
        Some((w, r)) => (w, r.trim()),
        None => (line, ""),
    };
    let no_arg = |c: Command| {
        if rest.is_empty() {
            Ok(c)
        } else {
            Err(format!("{word} takes no argument"))
        }
    };
    match word {
        "" => Err("empty command".into()),
        "q" | "quit" => no_arg(Command::Quit),
        "q!" | "quit!" => no_arg(Command::QuitForce),
        "e" | "edit" => {
            if rest.is_empty() {
                Err("usage: e <path>".into())
            } else {
                Ok(Command::Open(rest.to_string()))
            }
        }
        "version" => no_arg(Command::Version),
        "debug" => no_arg(Command::DebugToggle),
        w if w.chars().all(|c| c.is_ascii_digit()) => match w.parse::<usize>() {
            Ok(n) if n > 0 => no_arg(Command::PageGoto(n)),
            _ => Err(format!("no page {w}")),
        },
        w => match Command::from_name(w) {
            Some(c) => no_arg(c),
            None => Err(format!("unknown command: {w}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for (name, cmd, _) in REGISTRY {
            assert_eq!(Command::from_name(name).as_ref(), Some(cmd));
            assert_eq!(cmd.name(), *name);
        }
    }

    #[test]
    fn parses_ex_commands() {
        assert_eq!(parse_ex("q"), Ok(Command::Quit));
        assert_eq!(parse_ex(" q! "), Ok(Command::QuitForce));
        assert_eq!(
            parse_ex("e ~/papers/a b.pdf"),
            Ok(Command::Open("~/papers/a b.pdf".into()))
        );
        assert_eq!(parse_ex("42"), Ok(Command::PageGoto(42)));
        assert_eq!(parse_ex("zoom.in"), Ok(Command::ZoomIn));
        assert!(parse_ex("0").is_err());
        assert!(parse_ex("e").is_err());
        assert!(parse_ex("q now").is_err());
        assert!(parse_ex("nonsense").is_err());
    }
}

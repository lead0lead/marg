//! Named commands. Keys, `:` commands and the mouse all run these.

use std::fmt;

use crate::caret::Motion;
use crate::config::Layout;

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
    /// One screen down; single/spread: at the bottom, the next page.
    ScreenDown,
    /// One screen up; single/spread: at the top, the previous page.
    ScreenUp,
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
    /// Write the commented default config, if there is no config file.
    WriteConfig,
    /// `:export md [path]`: write the annotations as Markdown.
    Export(Option<String>),
    Version,
    DebugToggle,
    /// Hints on words; the picked word gets the caret.
    HintCaret,
    /// Hints on two words; selects from the first to the second.
    HintSelect,
    /// Hints on the visible annotations; selects the picked one.
    HintAnnot,
    /// Hints on the visible links; follows the picked one.
    HintLinks,
    /// Move the caret (Caret and Visual mode).
    Motion(Motion),
    /// Start a character-wise selection, or switch to it; in a
    /// character-wise selection, back to Caret mode.
    VisualChar,
    /// The same for a line-wise selection.
    VisualLine,
    /// Swap the moving end of the selection.
    VisualSwap,
    /// Highlight the selection with category 1–5.
    Highlight(u8),
    /// Underline the selection in the last-used category.
    Underline,
    /// Ask for the category, then highlight the selection.
    PickCategory,
    /// Copy the selected text.
    Copy,
    Undo,
    Redo,
    /// Save now (`:w`).
    Save,
    /// Save marg's version even if the file changed on disk (`:w!`).
    SaveForce,
    /// Reload from disk, discarding unsaved changes (`:e!`).
    Reload,
    /// Save, close the file and show the recent files list.
    Close,
    /// Select the next annotation in reading order.
    AnnotNext,
    /// Select the previous annotation.
    AnnotPrev,
    /// Delete the selected annotation.
    AnnotDelete,
    /// Remove highlights and underlines from the selected text (parts of
    /// them outside the selection stay).
    Erase,
    /// Delete every highlight and underline the selection touches, whole.
    EraseWhole,
    /// Give the selected annotation category 1–5.
    AnnotRecolor(u8),
    /// Switch the selected annotation between highlight and underline.
    AnnotToggleType,
    /// Edit the comment of the selected annotation.
    AnnotEdit,
    /// Highlight the selection in the last-used category and open the
    /// comment editor on it.
    Comment,
    /// A sticky note at the caret (or the top left of the page), with the
    /// comment editor open.
    Note,
    /// Box mode: draw a box with grid hints or the mouse.
    BoxMode,
    /// Open the annotation list.
    AnnotList,
    /// Switch dark (recolored) pages on or off.
    Recolor,
    /// `:layout continuous|single|spread`.
    SetLayout(Layout),
    /// Continuous → single → spread → continuous.
    LayoutCycle,
    /// Spread layout: show the first page alone (a book's cover), or not.
    SpreadFirstAlone,
    /// Back to the position before the last jump.
    JumpBack,
    /// Undo a `JumpBack`.
    JumpForward,
    /// Set the mark named by the next key (`a`–`z`).
    MarkSet,
    /// Go to the mark named by the next key.
    MarkJump,
    /// Show the table of contents.
    Outline,
    /// Open the search prompt (`/`).
    Search,
    /// Go to the next search hit.
    SearchNext,
    /// Go to the previous search hit.
    SearchPrev,
    /// The keys of the current mode (`?`).
    Help,
    /// The help page with everything (`:help`).
    HelpPage,
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
    (
        "scroll.screen_down",
        Command::ScreenDown,
        "screen down (single/spread: then next page)",
    ),
    (
        "scroll.screen_up",
        Command::ScreenUp,
        "screen up (single/spread: then previous page)",
    ),
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
    ("hint.caret", Command::HintCaret, "hint: place caret"),
    ("hint.select", Command::HintSelect, "hint: select range"),
    ("hint.annot", Command::HintAnnot, "hint: select annotation"),
    ("hint.links", Command::HintLinks, "hint: follow link"),
    (
        "caret.char_next",
        Command::Motion(Motion::CharNext),
        "next character",
    ),
    (
        "caret.char_prev",
        Command::Motion(Motion::CharPrev),
        "previous character",
    ),
    (
        "caret.word_next",
        Command::Motion(Motion::WordNext),
        "next word",
    ),
    (
        "caret.word_prev",
        Command::Motion(Motion::WordPrev),
        "previous word",
    ),
    (
        "caret.word_end",
        Command::Motion(Motion::WordEnd),
        "end of word",
    ),
    (
        "caret.word_end_prev",
        Command::Motion(Motion::WordEndPrev),
        "end of previous word",
    ),
    (
        "caret.line_start",
        Command::Motion(Motion::LineStart),
        "start of line",
    ),
    (
        "caret.line_end",
        Command::Motion(Motion::LineEnd),
        "end of line",
    ),
    (
        "caret.line_down",
        Command::Motion(Motion::LineDown),
        "line down",
    ),
    ("caret.line_up", Command::Motion(Motion::LineUp), "line up"),
    (
        "caret.sentence_prev",
        Command::Motion(Motion::SentencePrev),
        "previous sentence",
    ),
    (
        "caret.sentence_next",
        Command::Motion(Motion::SentenceNext),
        "next sentence",
    ),
    (
        "caret.block_prev",
        Command::Motion(Motion::BlockPrev),
        "previous paragraph",
    ),
    (
        "caret.block_next",
        Command::Motion(Motion::BlockNext),
        "next paragraph",
    ),
    ("visual.char", Command::VisualChar, "select characters"),
    ("visual.line", Command::VisualLine, "select lines"),
    ("visual.swap", Command::VisualSwap, "swap selection end"),
    (
        "annotate.highlight(1)",
        Command::Highlight(1),
        "highlight, category 1",
    ),
    (
        "annotate.highlight(2)",
        Command::Highlight(2),
        "highlight, category 2",
    ),
    (
        "annotate.highlight(3)",
        Command::Highlight(3),
        "highlight, category 3",
    ),
    (
        "annotate.highlight(4)",
        Command::Highlight(4),
        "highlight, category 4",
    ),
    (
        "annotate.highlight(5)",
        Command::Highlight(5),
        "highlight, category 5",
    ),
    ("annotate.underline", Command::Underline, "underline"),
    (
        "annotate.pick",
        Command::PickCategory,
        "highlight: choose category",
    ),
    ("selection.copy", Command::Copy, "copy text"),
    ("undo", Command::Undo, "undo"),
    ("redo", Command::Redo, "redo"),
    ("file.save", Command::Save, "save"),
    (
        "file.save_force",
        Command::SaveForce,
        "save, overwriting changes on disk",
    ),
    (
        "file.reload",
        Command::Reload,
        "reload from disk, discarding changes",
    ),
    (
        "file.close",
        Command::Close,
        "close the file, show recent files",
    ),
    ("annot.next", Command::AnnotNext, "next annotation"),
    ("annot.prev", Command::AnnotPrev, "previous annotation"),
    (
        "annot.delete",
        Command::AnnotDelete,
        "delete the selected annotation",
    ),
    (
        "annotate.erase",
        Command::Erase,
        "remove highlights from the selection",
    ),
    (
        "annotate.erase_whole",
        Command::EraseWhole,
        "delete highlights the selection touches",
    ),
    (
        "annot.recolor(1)",
        Command::AnnotRecolor(1),
        "recolor: category 1",
    ),
    (
        "annot.recolor(2)",
        Command::AnnotRecolor(2),
        "recolor: category 2",
    ),
    (
        "annot.recolor(3)",
        Command::AnnotRecolor(3),
        "recolor: category 3",
    ),
    (
        "annot.recolor(4)",
        Command::AnnotRecolor(4),
        "recolor: category 4",
    ),
    (
        "annot.recolor(5)",
        Command::AnnotRecolor(5),
        "recolor: category 5",
    ),
    ("annot.edit", Command::AnnotEdit, "edit comment"),
    ("annotate.comment", Command::Comment, "highlight + comment"),
    ("annotate.note", Command::Note, "new note"),
    ("annotate.box", Command::BoxMode, "box mode"),
    ("annot.list", Command::AnnotList, "annotation list"),
    ("view.recolor", Command::Recolor, "dark pages on/off"),
    ("view.layout_cycle", Command::LayoutCycle, "next layout"),
    ("jump.back", Command::JumpBack, "jump back"),
    ("jump.forward", Command::JumpForward, "jump forward"),
    ("mark.set", Command::MarkSet, "set mark a-z"),
    ("mark.jump", Command::MarkJump, "go to mark a-z"),
    ("outline", Command::Outline, "table of contents"),
    ("search", Command::Search, "search"),
    ("search.next", Command::SearchNext, "next search hit"),
    ("search.prev", Command::SearchPrev, "previous search hit"),
    ("help", Command::Help, "keys of this mode"),
    ("help.page", Command::HelpPage, "help page"),
    (
        "config.write",
        Command::WriteConfig,
        "write the default config file",
    ),
    (
        "view.spread_first_alone",
        Command::SpreadFirstAlone,
        "spread: first page alone on/off",
    ),
    (
        "annot.toggle_type",
        Command::AnnotToggleType,
        "switch highlight / underline",
    ),
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
            Command::Export(_) => "file.export",
            Command::SetLayout(_) => "view.layout",
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
/// `q`, `q!`, `e <path>`, `export md [path]`, `layout <name>`, `version`, `debug`, a page
/// number, or any registered command name (`zoom.in`).
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
        "w" | "write" => no_arg(Command::Save),
        "w!" | "write!" => no_arg(Command::SaveForce),
        "e!" | "edit!" => no_arg(Command::Reload),
        "close" | "bd" => no_arg(Command::Close),
        "q!" | "quit!" => no_arg(Command::QuitForce),
        "e" | "edit" => {
            if rest.is_empty() {
                Err("usage: e <path>".into())
            } else {
                Ok(Command::Open(rest.to_string()))
            }
        }
        "export" => match rest.split_once(char::is_whitespace) {
            None if rest == "md" => Ok(Command::Export(None)),
            Some(("md", path)) => Ok(Command::Export(Some(path.trim().to_string()))),
            _ => Err("usage: export md [path]".into()),
        },
        "layout" => match rest {
            "continuous" => Ok(Command::SetLayout(Layout::Continuous)),
            "single" => Ok(Command::SetLayout(Layout::Single)),
            "spread" => Ok(Command::SetLayout(Layout::Spread)),
            _ => Err("usage: layout continuous|single|spread".into()),
        },
        "help" | "h" => no_arg(Command::HelpPage),
        "config" => no_arg(Command::WriteConfig),
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
        assert_eq!(parse_ex("w"), Ok(Command::Save));
        assert_eq!(parse_ex("w!"), Ok(Command::SaveForce));
        assert_eq!(parse_ex("e!"), Ok(Command::Reload));
        assert_eq!(parse_ex("close"), Ok(Command::Close));
        assert_eq!(parse_ex("bd"), Ok(Command::Close));
        assert_eq!(parse_ex("annotate.highlight(3)"), Ok(Command::Highlight(3)));
        assert_eq!(parse_ex("zoom.in"), Ok(Command::ZoomIn));
        assert_eq!(
            parse_ex("layout spread"),
            Ok(Command::SetLayout(Layout::Spread))
        );
        assert!(parse_ex("layout").is_err());
        assert_eq!(parse_ex("export md"), Ok(Command::Export(None)));
        assert_eq!(
            parse_ex("export md ~/notes/a b.md"),
            Ok(Command::Export(Some("~/notes/a b.md".into())))
        );
        assert!(parse_ex("export").is_err());
        assert!(parse_ex("export html").is_err());
        assert_eq!(parse_ex("help"), Ok(Command::HelpPage));
        assert_eq!(parse_ex("config"), Ok(Command::WriteConfig));
        assert!(parse_ex("layout wide").is_err());
        assert!(parse_ex("0").is_err());
        assert!(parse_ex("e").is_err());
        assert!(parse_ex("q now").is_err());
        assert!(parse_ex("nonsense").is_err());
    }
}

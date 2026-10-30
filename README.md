# marg

A fast, keyboard-driven PDF reader and annotator for Linux, Windows and
macOS.
Zathura's speed and vim-style keys, plus real annotations: highlights,
underlines, comments, notes and boxes, written into the PDF as standard
annotations that Okular, Acrobat, browsers and Zotero can read.

- **Keyboard first.** Every action has a key, and every key can be remapped.
  Hint labels pick a line, then a word, so selecting text takes a few
  keystrokes. The mouse works too.
- **The PDF is the database.** Annotations are saved incrementally into the
  file, with no sidecar files. Before marg first changes a file, it keeps a
  backup of the original.
- **Reading.** Continuous, single-page and two-page layouts, an outline,
  search, marks, jump history, links, dark pages and themes.
- **Export.** `:export md` writes your highlights and comments as Markdown,
  grouped by category.

## Install

Release builds for Linux, Windows and macOS (Apple Silicon) are attached
to the [GitHub releases](https://github.com/lead0lead/marg/releases). The
Windows `.exe` needs the Microsoft Visual C++ Redistributable (x64), which
most systems already have.

**macOS:** unzip `marg-macos-arm64.zip` and move `marg.app` to
Applications. The app is not signed with an Apple Developer ID, so macOS
blocks the first start: right-click `marg.app` → Open → Open, or run
`xattr -dr com.apple.quarantine /Applications/marg.app` once. marg then
appears under "Open With" for PDFs (Finder, Zotero's file handling). For
the terminal: `ln -s /Applications/marg.app/Contents/MacOS/marg
/usr/local/bin/marg` (any directory on your `PATH`).

To build from source, you need stable Rust, a C compiler and libclang
(MuPDF is compiled from source by the `mupdf` crate). On Debian or Ubuntu:

```sh
sudo apt-get install libclang-dev libxkbcommon-dev libwayland-dev \
  libx11-dev libxcursor-dev libxrandr-dev libxi-dev libgl1-mesa-dev
cargo build --profile dist      # target/dist/marg
```

On macOS, the Xcode command line tools are enough
(`xcode-select --install`); `scripts/make-app.sh target/dist/marg .`
builds `marg.app`.

## Use

```sh
marg paper.pdf              # open a file
marg paper.pdf --page 12    # at page 12
marg                        # recent files
marg --help                 # options
```

Dropping PDFs on the window opens them: the first in that window, the
others in new windows (not on Wayland yet: the windowing library does not
support drag and drop there; XWayland works).

`?` shows the keys of the current mode, and `:help` shows everything,
including your own bindings.

**With Zotero:** open your PDFs in marg, for example by setting marg as the
program for PDFs in Zotero's settings (File Handling). Zotero picks up
marg's annotations through its "Import annotations" option.

**Annotations** are saved two seconds after a change and when you quit.
`:w` saves immediately. Backups of the originals are kept in
`~/.local/share/marg/backups` (Windows: `%LOCALAPPDATA%\marg\backups`,
macOS: `~/Library/Application Support/marg/backups`).
Encrypted PDFs ask for their password. PDFs whose permissions forbid
annotations open read-only; `:unlock` allows annotations in the open file.

## Configure

`:config` writes a commented config file with every setting and the
default keys:

| | Linux | Windows | macOS |
|---|---|---|---|
| Config | `~/.config/marg/config.toml` | `%APPDATA%\marg\config.toml` | `~/Library/Application Support/marg/config.toml` |
| State (positions, recent files) | `~/.local/state/marg/state.toml` | `%LOCALAPPDATA%\marg\state.toml` | `~/Library/Application Support/marg/state.toml` |
| Log | `~/.local/state/marg/marg.log` | `%LOCALAPPDATA%\marg\marg.log` | `~/Library/Application Support/marg/marg.log` |

marg reloads the config when you save it, and reports mistakes in the
status bar. Example themes are in [`docs/themes`](docs/themes).

**Web search:** `gs` in Visual mode looks the selected word, sentence or
paragraph up in your default browser, with DuckDuckGo unless you change
it. Add more engines (`%s` is the text) and bind them to keys:

```toml
[search.engines]
default = "https://www.google.com/search?q=%s"   # replaces DuckDuckGo
wiki = "https://en.wikipedia.org/wiki/Special:Search?search=%s"

[keys.visual]
gw = "selection.search(wiki)"
```

## Keys

The default keys (also in [`docs/cheatsheet.md`](docs/cheatsheet.md)):

<!-- cheat sheet: generated, MARG_BLESS=1 cargo test updates it -->

### Normal — moving

| Keys | |
|---|---|
| `j, Down` | scroll down |
| `k, Up` | scroll up |
| `h, Left` | scroll left |
| `l, Right` | scroll right |
| `Ctrl-d` | half page down |
| `Ctrl-u` | half page up |
| `J, PageDown` | next page |
| `K, PageUp` | previous page |
| `Space` | screen down (single/spread: then next page) |
| `Shift-Space` | screen up (single/spread: then previous page) |
| `gg, Home` | first page (or page n) |
| `G, End` | last page (or page n) |
| `+` | zoom in |
| `-` | zoom out |
| `=` | zoom 100% |
| `s` | fit width |
| `a` | fit page |
| `gl` | hint: follow link |
| `gr` | dark pages on/off |
| `Ctrl-l` | next layout |
| `Ctrl-o` | jump back |
| `Ctrl-i` | jump forward |
| `m` | set mark a-z |
| `'` | go to mark a-z |
| `o` | table of contents |
| `/` | search |
| `n` | next search hit |
| `N` | previous search hit |

### Normal — selecting

| Keys | |
|---|---|
| `f` | hint: place caret |
| `F` | hint: select range |

### Normal — annotating

| Keys | |
|---|---|
| `ga` | hint: select annotation |
| `u` | undo |
| `Ctrl-r` | redo |
| `]a` | next annotation |
| `[a` | previous annotation |
| `x` | delete the selected annotation |
| `i` | new note |
| `b` | box mode |
| `A` | annotation list |

### Normal — files

| Keys | |
|---|---|
| `:` | command line |
| `q` | quit |
| `Ctrl-w` | close the file, show recent files |
| `?` | keys of this mode |

### Caret — selecting

| Keys | |
|---|---|
| `l, Right` | next character |
| `h, Left` | previous character |
| `w` | next word |
| `b` | previous word |
| `e` | end of word |
| `ge` | end of previous word |
| `0` | start of line |
| `$` | end of line |
| `j, Down` | line down |
| `k, Up` | line up |
| `(` | previous sentence |
| `)` | next sentence |
| `{` | previous paragraph |
| `}` | next paragraph |
| `v` | select characters |
| `V` | select lines |

### Visual — selecting

| Keys | |
|---|---|
| `o` | swap selection end |
| `y` | copy text |
| `gs` | search the web for the text |

### Visual — annotating

| Keys | |
|---|---|
| `1-5` | highlight, category 1-5 |
| `U` | underline |
| `Enter` | highlight: choose category |
| `d` | remove highlights from the selection |
| `D` | delete highlights the selection touches |
| `c` | highlight + comment |

### Selected annotation — annotating

| Keys | |
|---|---|
| `1-5` | recolor: category 1-5 |
| `e` | edit comment |
| `U` | switch highlight / underline |

### Hints

| Keys | |
|---|---|
| `label` | pick that target |
| `Backspace` | remove the last typed character |
| `Esc` | cancel |

Text hints come in two stages: every visible line gets a label at its left, then each word of the picked line gets one. f puts the caret on the word, F selects from one word to another.

### Box mode

| Keys | |
|---|---|
| `label label` | the cells of two corners |
| `h j k l` | move the bottom-right corner (Ctrl: finely) |
| `H J K L` | move the top-left corner (Ctrl-Shift: finely) |
| `s` | snap to the image under the box |
| `1-5` | category |
| `Enter / Esc` | save / cancel |

### Comment editor

| Keys | |
|---|---|
| `Enter` | new line |
| `Ctrl-Enter` | save |
| `Esc` | cancel (twice when the text changed) |

### Lists (recent files, outline, annotations, help)

| Keys | |
|---|---|
| `j / k, Ctrl-n / Ctrl-p` | move |
| `gg / G` | first / last |
| `Ctrl-d / Ctrl-u` | half a page |
| `/` | filter; Enter opens the best match |
| `Enter` | open |
| `Esc` | clear the filter, then close |
| `recent files: :` | command line, e.g. :e path |
| `annotations: 1-5, 0` | one category, all |
| `annotations: e, x` | edit the comment, delete |

### Search prompt

| Keys | |
|---|---|
| `text` | search as you type (case matters only with capitals) |
| `Enter` | keep the search; n / N go to the next / previous hit |
| `Enter on an empty prompt` | search for the last query again |
| `Esc` | back to where the search started |
| `Esc in Normal mode` | hide the hits (n searches again) |

### Commands (:)

| Keys | |
|---|---|
| `:w, :w!` | save; save over changes made by another program |
| `:q, :q!` | save and quit; quit without saving |
| `:e path, :e!` | open a file (Tab completes); reload from disk |
| `:close, :bd` | close the file, show the recent files |
| `:layout continuous\|single\|spread` | set the layout |
| `:export md [path]` | write the annotations as Markdown (default: &lt;file&gt;.notes.md) |
| `:42` | go to page 42 |
| `:help` | the help page |
| `:config` | write a commented default config, if there is none |
| `:set name=value, :set name` | change a setting for this session (e.g. :set zoom_step=1.2); show it |
| `:unlock` | annotate this file although its permissions forbid it |
| `:version, :debug` | version; debug overlay |
| `:&lt;command name&gt;` | run any command by name, e.g. :zoom.in |
| `Ctrl-V` | paste (also in search, password and filters) |

### Categories

| Keys | |
|---|---|
| `1` | Key point |
| `2` | Question |
| `3` | Definition |
| `4` | Disagree |
| `5` | Evidence |

The category name is written into the annotation's subject; other programs' annotations get a category by subject, else by the nearest color.
<!-- end of cheat sheet -->

## Licence

AGPL-3.0, like MuPDF, which marg is built on.

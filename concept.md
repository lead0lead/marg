# marg — Concept & Build Plan

> marg is a lightweight, very fast PDF reader and annotator for Linux and
> Windows, driven almost entirely from the keyboard. It combines Zathura's
> speed and vim-style control with real annotations (highlights, underlines,
> comments, boxes) that are written into the PDF as standard annotations.

This document is the **source of truth** for the project and the starting point
for every Claude Code session. Keep it up to date whenever a feature ships or a
decision is made.

Full title: **marg** (from "margin"). Short name for repo, binary, config
folder and state keys: `marg`.

---

## Start Here — Instructions for Claude Code

When the user says something like "look at concept.md and start" or "continue",
do this, in order:

1. **Read this whole document.** Then read §0 (current state), the last entries
   of the session log, and the open questions again; they tell you where the
   project stands.
2. **Check the repository state:** `git status`, `git log --oneline -15`, the
   current branch, the commit author identity, and whether build and tests
   pass (once the project exists).
3. **Tell the user in a few sentences** where the project stands and what you
   will do next, according to the build phases. If §0 and the repository
   disagree, say so and trust the repository.
4. **Work on the next unfinished phase only.** If the repository is empty apart
   from this file, that is Phase 0. Work on a feature branch, one commit per
   step.
5. **Stop at the end of each phase** and hand the build to the user to try.
   Never go past the decision milestone named in §0 without the user's
   explicit go-ahead.
6. **Follow the repository rules in "How We Work" without exception.**
7. **Ask instead of guessing** when something here is unclear, contradictory,
   or marked as an open decision. Ask one question at a time.
8. **Follow "When Things Go Wrong"** as soon as a problem resists a first fix.

The user is an experienced developer: be concise and technical, skip basic
explanations.

**Keeping this document current is part of the job.** A new session starts
with no memory of earlier ones; this file and the git history are all it has.

- After every completed step that changes the state of the project, update §0
  in the same step.
- At the end of every phase, and before ending any session (also when the user
  says they are stopping, or when the context is getting full), update:
  - §0: status, current phase, next step, live version.
  - The session log: add an entry (format given there).
  - The relevant content sections, so they describe the project **as built**.
    Mark implemented sections with "(as implemented)" in their heading.
  - Open questions: add new ones and known issues; remove resolved ones.
- Record every decision the user makes (an approved value, a design change, a
  rejected idea) in the relevant section and in the session log, with the
  reason when one was given.
- Never delete the plan for phases that are not built yet.
- This document is committed to git. Commit document updates together with the
  code they describe.

---

## 0. Current State (read this first)

| | |
|---|---|
| **Status** | Phase 1 (viewer) in progress on branch `phase-1-viewer`. Done: step 1 (config defaults in `marg-core/src/config.rs`, paths, logging, config loading); step 2 (document thread `marg-pdf/src/thread.rs`: open, renders of pages and tiles, replace-queue semantics, generations); step 3 (`marg-core/src/view.rs`: continuous layout, scroll, zoom with anchor, fit width/page, page navigation); step 4 (viewer: `marg/src/pages.rs` texture LRU (`marg-core/src/lru.rs`), pre-render ±2, nearest-scale fallback, 512 px tiles above 200% over a low-res page; mouse wheel / Ctrl+wheel); steps 5+6 together (`marg-core` `keys.rs`, `command.rs`, `keymap.rs`, `lineedit.rs`: key notation, command registry, Normal keymap with `[keys.normal]` remaps, counts, sequences with timeout, `:` command line with history and `:e` Tab completion; status bar with pending keys and messages). |
| **Current phase** | 1 — viewer |
| **Next step** | Phase 1 step 7: state file + recent files. Plan: 1 config/paths/logging ✅ · 2 document thread ✅ · 3 continuous layout + zoom/fit model in core ✅ · 4 viewer with texture cache, pre-render, scaled fallback, tiles ✅ · 5 commands + keymap + counts ✅ · 6 status bar + command line ✅ · 7 state file + recent files · 8 debug overlay + startup timing |
| **Decision milestone** | End of Phase 2: viewer + hint/caret selection + highlights that save into the PDF. The user reads real papers with it for about a week and decides whether selection is fast enough. Do not build Phase 3+ without the user's go-ahead. |
| **Live version** | none yet |
| **Repository** | private GitHub repository `marg`, branch `main` |
| **App** | runs locally: `cargo run --release -- <file.pdf> [--page N]` (Phase 1 in progress: continuous viewer) |
| **Target** | Linux (X11 and Wayland) and Windows 10/11, desktop only. Out of scope: macOS, mobile, tablets, pen input. |
| **Last updated** | 2026-10-27 (Phase 1 steps 5+6) |

---

## 1. Core Concept

marg is a personal daily-driver reader for slide decks, papers and book
chapters. The user manages files in Zotero and opens them in marg (directly or
as Zotero's external PDF viewer). marg is standalone: it does not read or
write Zotero's database. Its annotations live inside the PDF as standard PDF
annotations, so Okular, Acrobat, browsers, and Zotero (via "Import
annotations") can see them.

Reference points: **Zathura** (speed, minimal UI, vim keys; lacks
annotation), **Sioyek** (keyboard-driven paper reading), **Vimium** (hint
labels for keyboard selection).

**Design pillars**

- **Fast above all.** No action may feel slower than Zathura. Startup, page
  turns and keystrokes have budgets (see Targets), and features that break
  them are redesigned, not shipped.
- **Keyboard-first, mouse works.** Every action has a key. The mouse (drag to
  select, click, wheel) works as a fallback, never as the only way.
- **Everything is a command.** Keys, `:commands`, and mouse actions all call
  named commands. Every key binding is remappable in a config file.
- **The PDF is the database.** Annotations are standard PDF annotations saved
  incrementally. No sidecar files hold annotation data.
- **Never lose the user's work or files.** A backup of the original is kept
  before the first modification. Unsaved changes are never silently discarded
  or overwritten.

**Targets (starting values, measured in the debug overlay)**

- Cold start to first rendered page: < 150 ms for a typical 20-page paper on
  the user's machine.
- Page turn: within one frame (≤ 16 ms) when the page is pre-rendered; < 50 ms
  when not.
- Keystroke to visible response: within one frame.
- Memory: < 300 MB with a 500-page book open.
- Binary: a single executable per platform with no runtime dependencies
  besides the OS graphics stack.

---

## 2. Core Mechanics / Features

All numbers marked *(start)* are starting values to tune. They live in the
defaults of `config.toml` (see §8), not scattered in code.

### 2.1 Opening files

- `marg <file.pdf> [--page N]` opens one file in one window. One window per
  file, no tabs. The window manager handles multiple documents.
- `marg` with no argument shows a **recent files** list (from the state file,
  most recent first, max 50 *(start)*), fuzzy-filterable; Enter opens.
- `:e <path>` opens another file in the same window, with path completion on
  Tab. The previous file is saved first if it has pending changes.
- Password-protected PDFs: prompt for the password in the command line. PDFs
  whose permissions forbid annotation open read-only, with a status message.
- Files that fail to open: show the MuPDF error in the status bar and in the
  log. Never crash.

### 2.2 Rendering and caching

- One **document thread** owns the MuPDF document. It handles all rendering,
  text extraction, and annotation edits. The UI thread never calls MuPDF. The
  two communicate by channels (requests and results).
- Pre-render ±2 pages *(start)* around the visible area, at the current zoom
  and device pixel ratio. Keep rendered pages in an LRU texture cache with a
  256 MB budget *(start)*.
- When zoom exceeds 200% *(start)*, render in 512 px tiles and only render
  visible tiles plus a one-tile margin.
- While a page renders, show the nearest cached version scaled (blurry but
  instant). Never show a blank page if any version exists.
- The structured text of each page (MuPDF stext: blocks, lines, characters
  with boxes) is extracted lazily and cached per page. Hints, caret motions,
  selection, search, and export all use it.
- After an annotation edit, re-render only the affected page. Until the new
  render arrives, the UI draws the new annotation as a provisional overlay, so
  feedback is instant.
- Measure first: if one document thread cannot meet the targets, consider
  extra read-only render workers (each with its own document handle). Ask the
  user before adding that complexity.

### 2.3 Layouts

Three layouts, switchable per document. The current layout is remembered per
file.

| Layout | Pages | `J` / `K` | `j` / `k` |
|---|---|---|---|
| Continuous | Stacked vertically, 8 px *(start)* gap | Next / previous page top | Scroll 60 px *(start)* |
| Single (slides) | One page, centered | Next / previous page | Scroll inside a zoomed page; at the page edge, do nothing (no spill-over) |
| Spread | Two pages side by side | Next / previous spread | As Single |

- Default layout for a file opened the first time: if the first page is
  landscape (width > height), use Single with fit-page; otherwise Continuous
  with fit-width. Saved per-file state overrides this.
- Spread option `spread_first_page_alone` (cover shown alone, for books),
  toggled with a command. Default `false` *(start)*.
- Zoom: factor 1.1 per step *(start)*, range 10%–800%. Fit-width and
  fit-page modes stay active across window resizes until the user zooms
  manually.
- No scroll animation by default (`scroll_smooth = false`). Instant is the
  point.

### 2.4 Navigation

- Counts work for motions and page jumps: `5j`, `3J`, `42G`.
- Half-page scroll with `Ctrl-d` / `Ctrl-u`.
- **Jump history**: `gg`, `G`, `NG`, search jumps, outline jumps, mark jumps,
  and link follows push the previous position onto a jump list (max 100
  *(start)*). `Ctrl-o` goes back, `Ctrl-i` forward.
- **Marks**: `m{a-z}` sets, `'{a-z}` jumps. Marks are per file and persist in
  the state file.
- **Links**: internal PDF links can be followed with the mouse or with link
  hints (`gl`, labels on visible links). External URLs open in the default
  browser only after a confirmation in the status bar (`y` to confirm).
- **Outline**: `o` opens the PDF's table of contents as an overlay list,
  indented by level. Typing filters it (fuzzy). `j/k` or `Ctrl-n/p` move,
  Enter jumps, Esc closes. If the PDF has no outline, show a status message.

### 2.5 Search

- `/` opens incremental search in the command line. Smartcase: insensitive
  unless the query contains an uppercase letter.
- Matching runs on the document thread, page by page from the current page,
  and results stream in. Hits on visible pages are highlighted; the current
  hit is emphasised.
- `n` / `N` go to the next or previous hit. The status bar shows `hit 3/27`
  (or `3/27+` while counting).
- Hyphenated line breaks are joined when matching (MuPDF dehyphenation), and
  ligatures match their plain letters.
- No hits: status message, position unchanged.

### 2.6 Hints (keyboard targeting)

Hint labels are drawn over targets; typing a label picks that target.

- Label alphabet: `hint_chars = "asdfghjkl"` *(start)*. Labels are prefix-free
  and as short as possible for the number of targets. Backspace removes the
  last typed character; Esc cancels.
- **Text hints are two-stage** (avoids an unreadable label on every word):
  1. Every visible text line gets a label at its left edge.
  2. After picking a line, each word in that line gets a single-character
     label (alphabet extended with `qwertyuiop` for long lines).
  Typical cost: 3 keystrokes to reach any visible word.
- Config option `hint_mode = "two-stage" | "words"`: `words` labels every
  visible word directly, for users who prefer one stage.
- `f`: pick a word, which places the **caret** at its start (Caret mode).
- `F`: pick a start word, then an end word, which creates a **selection** from
  the start of the first to the end of the second (either order works).
- `gl`: hints on visible links. `ga`: hints on visible annotations (selects
  the picked annotation).
- Pages without a text layer (scans): `f` / `F` show "no text on this page —
  use a box (b)" instead of empty hints.

### 2.7 Caret and visual selection

- **Caret mode** shows a 2 px caret bar at a character position. Motions
  follow MuPDF's reading order of the structured text:
  - `w` / `b` / `e` / `ge`: word motions, as in vim.
  - `0` / `$`: start or end of line.
  - `j` / `k`: line down or up, keeping the horizontal position.
  - `(` / `)`: previous or next sentence (split on `.`, `?`, `!` followed by
    whitespace; abbreviations are not special-cased in v1).
  - `{` / `}`: previous or next text block (paragraph).
  - Counts work: `3w`.
  - When the caret leaves the visible area, the view scrolls to keep it
    visible.
- `v` starts a character-wise selection at the caret. `V` starts a line-wise
  selection. Motions extend it. `o` swaps the moving end.
- The selection is drawn as translucent blue over the selected character
  boxes, line by line.
- **Mouse**: drag selects text character-wise. Double-click selects a word.
  Triple-click selects a line. Mouse selection behaves exactly like a keyboard
  selection afterwards.
- `y` copies the selected text to the clipboard (dehyphenated, lines joined
  with spaces).
- Esc clears the selection and leaves Caret mode.
- Multi-column papers rely on MuPDF's block order. If the order is wrong on a
  page, motions follow the wrong order; that is a known risk (§12), checked at
  the decision milestone.

### 2.8 Annotations

All annotations are created on the document thread through MuPDF's PDF
annotation API and become standard PDF annotation objects.

**Color categories.** Five categories, each with a name and a color, set in
`config.toml`. Defaults *(start, user to confirm)*:

| Key | Name | Color |
|---|---|---|
| 1 | Key point | yellow `#FFD400` |
| 2 | Question | purple `#A28AE5` |
| 3 | Definition | blue `#2EA8E5` |
| 4 | Disagree | red `#FF6666` |
| 5 | Evidence | green `#5FB236` |

The category name is written to the annotation's Subject (`/Subj`) field.
When reading annotations created elsewhere, the category comes from `/Subj`
if it matches a name, else from the nearest category color, else
"Uncategorised". The annotation author (`/T`) is `annotation_author` from the
config (default: the OS user name).

**Types**

| Type | PDF subtype | Created by | Details |
|---|---|---|---|
| Highlight | Highlight | `1`–`5` on a selection | One QuadPoint per selected line, category color, opacity 0.4 *(start)* |
| Underline | Underline | `U` on a selection | Last-used category color |
| Comment | Highlight + `/Contents` | `c` on a selection | Highlight in last-used category, then the comment editor opens |
| Note | Text (sticky note) | `i` | At the caret, or at the top-left page margin if there is no caret; comment editor opens immediately |
| Box | Square | `b` (box mode) | Stroke 1.5 pt *(start)*, category color, no fill |

**Box mode** (`b`):
1. A grid (9 columns × 12 rows *(start)*) is laid over the visible page, each
   cell labeled.
2. The first label sets the box's top-left corner (top-left of that cell); the
   second sets the bottom-right corner (bottom-right of that cell).
3. Adjust: `h/j/k/l` move the bottom-right corner, `H/J/K/L` the top-left
   corner, by 4 pt *(start)*; with `Ctrl` by 1 pt. `s` snaps the box to the
   bounds of the image block it overlaps most, if there is one.
4. `1`–`5` set the category, Enter confirms, Esc cancels.

With the mouse, in box mode, drag a rectangle instead of steps 1–3.

**Working with existing annotations**

- `]a` / `[a` select the next or previous annotation in page order; `ga` picks
  one with hints; clicking an annotation selects it. A selected annotation
  gets a dashed outline.
- On a selected annotation: `e` edits its comment, `x` deletes it, `1`–`5`
  recolor it, `U` toggles highlight ↔ underline.
- Hovering an annotation with a comment, or selecting it, shows the comment
  as a tooltip next to it.
- **Undo / redo**: `u` and `Ctrl-r` undo and redo annotation operations
  (create, delete, edit comment, recolor, change type), 100 steps *(start)*,
  for the current session only.

**Comment editor**

- An inline popup anchored next to the annotation, 360 px wide *(start)*,
  growing in height up to 40% of the window, then scrolling.
- Plain text. Enter inserts a newline. `Ctrl-Enter` saves. Esc cancels; if the
  text changed, the first Esc shows "Esc again to discard".
- Must handle the user's keyboard layout, including German umlauts and dead
  keys, on both Linux and Windows.

### 2.9 Saving and file safety

- **Incremental save**: changes are appended to the PDF using MuPDF's
  incremental write. Saving a 300-page book must not rewrite it.
- **Autosave** 2 s *(start)* after the last annotation change, and on quit.
  `:w` saves immediately.
- **Backup**: before the first modification of a file (no backup exists for
  it yet), copy the original to
  `<data_dir>/backups/<pdf-id>-<filename>`. The backup is never overwritten
  later. Backups are not placed next to the PDF, to keep Zotero's storage
  folders clean.
- If an incremental save is impossible (for example, MuPDF had to repair a
  broken file), do a full save to a temporary file in the same folder and
  atomically rename it over the original.
- If saving fails (permissions, file locked by another program on Windows):
  keep changes in memory, show the error in the status bar with a ● unsaved
  marker, and retry on the next autosave. Never drop the changes.
- **External changes**: check the file's modification time every 1 s
  *(start)*. If it changed and marg has no unsaved changes, reload and keep the
  position. If marg has unsaved changes, do not reload or overwrite; warn in
  the status bar. `:w!` forces marg's version, `:e!` reloads from disk and
  discards marg's changes.
- `q` / `:q` save pending changes, then quit. `:q!` quits without saving.

### 2.10 Annotation list

- `A` opens a side panel (35% of the window width *(start)*) listing all
  annotations in page order: color swatch, category, page number, the first
  80 characters of the highlighted text (or "[Box]" / "[Note]"), and the
  comment.
- Typing filters by text and comment (fuzzy). `1`–`5` filter by category;
  `0` clears the filter.
- `j/k` move, Enter jumps to the annotation and selects it, `e` edits the
  comment, `x` deletes. Esc or `A` closes the panel.

### 2.11 Markdown export

`:export md [path]` writes `<pdf-name>.notes.md` next to the PDF (or to
`path`). The file is generated and overwritten on each export; its first line
says so.

Format:

```markdown
<!-- Generated by marg from paper.pdf on 2026-10-27. Re-exporting overwrites this file. -->
# <PDF title from metadata, else file name>

## Key point

> The highlighted text, dehyphenated, lines joined.
> — p. 12

The comment, if any.

> Next highlight in this category…
> — p. 14

## Question

**[Box]** — p. 7

Comment on the box.
```

- Sections follow category order 1–5, then "Uncategorised"; empty categories
  are left out. Within a section, entries are in page order, then top-to-bottom.
- Page numbers use the PDF's page labels if present (e.g. "xiv"), else the
  1-based page index.
- Standalone notes appear as **[Note]** entries with their comment.

### 2.12 Help overlay

`?` shows the bindings active in the current mode, generated from the live
keymap (so remaps show up). Esc closes.

---

## 3. Controls & Interaction

### Modes

| Mode | Entered by | Left by | Status bar label |
|---|---|---|---|
| Normal | start, Esc | — | `NORMAL` |
| Hint | `f`, `F`, `gl`, `ga`, grid in box mode | pick or Esc | `HINT` |
| Caret | `f` pick, `i` placement | Esc | `CARET` |
| Visual | `v`, `V`, `F` pick, mouse drag | Esc, or an annotation action | `VISUAL` |
| Box | `b` | Enter or Esc | `BOX` |
| Command | `:`, `/` | Enter or Esc | `:` / `/` prompt |
| Insert (comment editor) | `c`, `e`, `i` | Ctrl-Enter or Esc | `INSERT` |
| List (outline, annotations, recent files) | `o`, `A`, start without file | Enter or Esc | `OUTLINE` / `ANNOTATIONS` / `RECENT` |

### Default keymap

All of these are names of commands bound in the default keymap; every one is
remappable (§8).

| Context | Keys | Command |
|---|---|---|
| Normal | `j` / `k`, `h` / `l` | scroll down / up, left / right |
| | `Ctrl-d` / `Ctrl-u` | half page down / up |
| | `J` / `K`, `Space` / `Shift-Space` | next / previous page (or view) |
| | `gg`, `G`, `{n}G` | first page, last page, page n |
| | `Ctrl-o` / `Ctrl-i` | jump back / forward |
| | `+` / `-` / `=` | zoom in / out / 100% |
| | `s` / `a` | fit width / fit page |
| | `:layout continuous\|single\|spread` and `Ctrl-l` | set / cycle layout |
| | `o` | outline |
| | `/`, `n` / `N` | search, next / previous hit |
| | `m{a-z}`, `'{a-z}` | set mark, jump to mark |
| | `f`, `F` | hint: caret, hint: select range |
| | `gl`, `ga` | hint: links, hint: annotations |
| | `]a` / `[a` | next / previous annotation |
| | `i` | new note |
| | `b` | box mode |
| | `A` | annotation list |
| | `u` / `Ctrl-r` | undo / redo |
| | `?` | help |
| | `q` | save and quit |
| Caret | `w b e ge 0 $ j k ( ) { }` | motions |
| | `v` / `V` | start visual / line-visual |
| Visual (selection) | motions | extend selection |
| | `o` | swap selection end |
| | `1`–`5` | highlight with category |
| | `U` | underline |
| | `c` | highlight + comment |
| | `y` | copy text |
| Selected annotation | `e`, `x`, `1`–`5`, `U` | edit comment, delete, recolor, toggle type |
| Command | `:w`, `:w!`, `:q`, `:q!`, `:e path`, `:e!`, `:export md`, `:layout …`, `:set key=value`, `:debug` | |

Note: `L` is deliberately unbound in Normal mode (it is used in Box mode for
corner moves); layout cycling uses `Ctrl-l`.

**Mouse**: wheel scrolls, `Ctrl`+wheel zooms around the cursor, drag selects
text (or draws a box in Box mode), click selects an annotation or follows a
link, double/triple-click selects a word/line, right-click on a selection or
annotation opens a small menu with the same actions as the keys (the only
context menu in the app).

**Interaction rules**

- Keys are matched on the **logical character** produced by the active
  keyboard layout (text input events), not physical key positions, so remaps
  work on any layout. Keys with modifiers (`Ctrl-o`) are matched on key plus
  modifiers.
- Multi-key sequences (`gg`, `]a`, `m{a}`) time out after 1000 ms *(start)*;
  a pending sequence is shown at the right of the status bar.
- Unknown keys do nothing and show nothing.

### Status bar

One line at the bottom, monospace:

```
NORMAL  paper.pdf  12/34  110%  continuous  ●          3/27   gg
```

Mode, file name, page/total (page label if present), zoom, layout, ● if
there are unsaved changes, then on the right: search hit count, pending key
sequence. Messages (info, warning, error) replace the right part for 4 s
*(start)*; errors are also written to the log.

---

## 4. Content & Structure

Screens and overlays (there are no other windows):

| Element | Purpose |
|---|---|
| Page view | The document in the current layout, with annotations, selection, caret, hints, provisional overlays |
| Status bar / command line | §3 |
| Outline overlay | Table of contents, filterable |
| Recent files list | Shown when started without a file |
| Annotation side panel | §2.10 |
| Comment popup | §2.8 |
| Help overlay | §2.12 |
| Debug overlay | §8 |

Data handled:

- **The PDF** (annotations inside it).
- **Config file** (user-edited): keymap, categories, defaults.
- **State file** (marg-managed): per-file reading position, zoom, layout,
  marks; recent files list.
- **Backups** of originals.
- **Log file.**

---

## 5. Story, Tone & Text

Not applicable. UI messages are short, plain English, without exclamation
marks (e.g. "No text on this page — use a box (b)", "Saved", "Save failed:
file is locked; will retry").

---

## 6. Look

Reference: Zathura's minimalism. No toolbar, no menu bar, no icons.

- Background around pages: neutral dark gray `#2B2B2B` *(start)*. Pages render
  as-is (white). Thin 1 px shadow line at page edges.
- Status bar: 22 px high *(start)*, monospace, light text on `#1E1E1E`.
- Hint labels: small rounded badges, black text on yellow `#FFD84D`, bold
  monospace; already-typed characters dimmed. Labels never cover each other
  in two-stage mode (line labels sit in the left margin of the line).
- Selection: translucent blue (`#3D7EFF` at 35%). Caret: 2 px red bar.
  Selected annotation: dashed 1 px outline in the annotation's color.
- Overlays (outline, list, help, comment popup): dark panels matching the
  status bar, monospace text, the current row highlighted.
- Fonts: egui's bundled default fonts. No extra font files in v1 (no licence
  bookkeeping needed). If a font is added later, record its licence in
  `ASSETS.md`.
- Dark/recolored page rendering is not in v1 (see §13).

---

## 7. Audio

Not applicable. marg makes no sounds.

---

## 8. UI, Progress & Saving

### Paths

| | Linux | Windows |
|---|---|---|
| Config | `~/.config/marg/config.toml` | `%APPDATA%\marg\config.toml` |
| State | `~/.local/state/marg/state.toml` | `%LOCALAPPDATA%\marg\state.toml` |
| Data (backups) | `~/.local/share/marg/backups/` | `%LOCALAPPDATA%\marg\backups\` |
| Log | `~/.local/state/marg/marg.log` | `%LOCALAPPDATA%\marg\marg.log` |

Resolve these with the `directories` crate (respecting `XDG_*` variables). A
missing config file means defaults; `:config` writes a fully commented default
config to the config path if none exists.

### Config sketch (`config.toml`)

```toml
[view]
default_layout_portrait = "continuous"   # continuous | single | spread
default_layout_landscape = "single"
page_gap_px = 8
scroll_step_px = 60
zoom_step = 1.1
scroll_smooth = false
spread_first_page_alone = false

[render]
prerender_pages = 2
cache_mb = 256
tile_above_zoom = 2.0

[hints]
chars = "asdfghjkl"
mode = "two-stage"     # two-stage | words

[annotation]
author = ""            # empty = OS user name
highlight_opacity = 0.4
box_stroke_pt = 1.5
autosave_secs = 2

[[category]]
name = "Key point"
color = "#FFD400"
# … categories 2–5

[keys.normal]
"j" = "scroll.down"
"gg" = "page.first"
"Ctrl-o" = "jump.back"
# … full default keymap, one line per binding

[keys.visual]
"1" = "annotate.highlight(1)"
"U" = "annotate.underline"
```

- The config reloads automatically when the file changes (checked every 1 s).
  Parse errors keep the old config and show the error with line number in the
  status bar.
- Keymap conflicts (two commands on one key in one mode, or a key that is a
  prefix of another) are reported at load; the later binding wins.
- `:set key=value` changes a setting for the current session only.

### State file

Keyed by the PDF's permanent document ID (first element of the trailer `/ID`
array), which stays the same across incremental saves. Fallback for PDFs
without an ID: the canonical absolute path. (Not a content hash: that changes
on every save.)

Stored per file: last page and offset, zoom and fit mode, layout, marks, last
opened time, last known path. Recent files list: the 50 most recent.

### Developer tooling

- `:debug` toggles an overlay: frame time (last / p95), render queue length,
  cache usage in MB, last render time per page, open file's MuPDF warnings.
- `MARG_LOG=debug` raises the log level. `MARG_TIMING=1` logs startup phases
  with timestamps (process start → window → document open → first page
  shown), used to check the startup target.
- `:version` shows the version (from `Cargo.toml`) and git commit hash.

---

## 9. Tech Stack & Code Structure

| Part | Choice | Why |
|---|---|---|
| Language | Rust (stable) | Single static binary on Linux and Windows; memory safety; the compiler catches mistakes early |
| PDF engine | MuPDF via the `mupdf` crate | Fastest mainstream renderer; full PDF annotation API with incremental save |
| UI | `eframe` / `egui` | Lightweight, GPU-drawn, cross-platform, easy custom overlays; no heavyweight widget toolkit needed for a minimal UI |
| Config/state | `serde` + `toml` | Human-editable config |
| Paths | `directories` | Correct per-OS config/state/data locations |
| Logging | `log` + `simplelog` (file + stderr) | Simple, file output |
| Clipboard | via egui/eframe's built-in clipboard | No extra dependency |
| Fuzzy filter | own small subsequence scorer in core | Lists are short; no dependency needed |
| Tests | `cargo test`; fixture PDFs in `tests/fixtures/` | Round-trip annotation tests |

Licence note: MuPDF is AGPL-3.0. That is fine for a private personal tool;
it only matters if marg is ever distributed. Recorded here so it isn't
forgotten.

**Ask the user before adding any library not listed here.**

### MuPDF API coverage (Phase 0 spike, as implemented)

Checked with `mupdf` 0.8.0 (built with default features off, only
`base14-fonts`), in `crates/marg-pdf/tests/spike.rs` on the generated
fixture `tests/fixtures/text.pdf`. The `highlight_copy` example's output
renders correctly in Poppler (`pdftoppm`), an independent renderer.

| Need | Status | How |
|---|---|---|
| Render with annotations | ✅ | `Page::to_pixmap(.., show_extras = true)` |
| Tiles | ✅ (not tested yet) | `Device::from_pixmap_with_clip` + `Page::run` |
| Cancel a render | ✅ (not tested yet) | `Cookie::abort` with `run_with_cookie` |
| Structured text with char boxes | ✅ | `to_text_page(flags).structured()`: blocks → lines → chars with `quad`, `origin`; page space, y down; `DEHYPHENATE` flag exists |
| Search | ✅ (not tested yet) | `Page::search`, `TextPage::search` |
| Highlight / Underline / Text / Square | ✅ | `PdfPage::add_*_annotation` |
| Color, opacity, QuadPoints, Contents, author, border width | ✅ | `PdfAnnotation` setters, all round-trip |
| `/Subj` | ⚠️ no setter | write via `annot.object().dict_put("Subj", ..)`; round-trips |
| Delete annotation | ✅ | `PdfPage::delete_annotation` |
| Incremental save | ✅ | `PdfWriteOptions::set_incremental(true)`; original bytes stay an untouched prefix |
| Can save incrementally / repaired | ✅ / ⚠️ | `can_be_saved_incrementally()`; `pdf_was_repaired` only in `mupdf-sys` (not needed: the first check covers the save rule) |
| Document ID | ⚠️ no helper | trailer `/ID[0]` via `trailer().get_dict("ID")`; stable across incremental save |
| Outline, links, page labels, metadata | ✅ | `outlines()`, `Page::links()` (internal dest page or URI), `PdfDocument::page_label`, `metadata()` |
| Password, permissions | ✅ (not tested yet) | `needs_password`, `authenticate`, `permissions` |

Page sizes at open are read from the page dictionaries (CropBox ∩ MediaBox,
Rotate, UserUnit) instead of loading every page: 1316 pages open in 18 ms
instead of 366 ms (release). Matches MuPDF's page bounds on all 51 PDFs
checked and on `tests/fixtures/boxes.pdf`; falls back to loading the page if
the boxes are missing. Renders draw onto a white RGBA pixmap (opaque, so no
conversion on the UI thread). Timing tool:
`cargo run --release -p marg-pdf --example open_timing -- file.pdf`.

Threading: MuPDF's context is thread-local and `Document`/`Page` are not
`Send`, so the document must be opened and used on the document thread
(matches §2.2). Only `DisplayList` is `Send + Sync`.

### Key technical rules

- **Cargo workspace with three crates:**
  - `marg-core`: pure logic, no MuPDF, no egui. Command registry and parser,
    keymap parsing and sequence matching, mode state machine, hint label
    generation, caret motions and selection over a plain text model (pages →
    blocks → lines → chars with rectangles), search matching, category
    mapping, Markdown export, fuzzy scorer, config/state types. Fully
    unit-tested.
  - `marg-pdf`: the MuPDF adapter and document thread: open, render, extract
    structured text into the core text model, create/edit/delete annotations,
    save, backups.
  - `marg` (app): eframe UI, input → commands, drawing, overlays, channels to
    the document thread.
- **The UI thread never blocks** on MuPDF, disk, or the clipboard for more
  than a frame. Everything slow goes through the document thread.
- **Coordinates**: page space in PDF points (1/72 in) everywhere in core;
  conversion to screen pixels happens only in the app layer.
- **All tuning values come from config defaults**, defined in one place in
  `marg-core` (`config.rs`).
- Release builds: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`;
  Windows built with the MSVC toolchain and the **dynamic** CRT: `mupdf-sys`
  builds MuPDF's own `platform/win32/mupdf.sln`, whose projects use `/MD`
  (Release) and `/MDd` (Debug), so `+crt-static` would mismatch. The `.exe`
  currently needs the MSVC runtime (see open questions).

### Folder structure

```
marg/
├── concept.md
├── CLAUDE.md
├── Cargo.toml              # workspace
├── crates/
│   ├── marg-core/src/      # commands, keymap, modes, hints, motions, export, config
│   ├── marg-pdf/src/       # mupdf adapter, document thread, save/backup
│   └── marg/src/           # main.rs, ui/, input/, overlays/
├── tests/fixtures/         # small PDFs: text, two-column, scanned page, slides, outline, links
├── .github/workflows/ci.yml
└── .gitignore
```

Fixture PDFs are generated by a script in the repo (or are the user's own
test files), so their licence is clear.

---

## 10. How We Work

### Repository rules

- **Private GitHub repository** `marg`. Everything is committed, including
  `concept.md` and `CLAUDE.md`.
- `.gitignore`:
  ```gitignore
  /target/
  *.marg-tmp
  .DS_Store
  CLAUDE.local.md
  ```
- Commit messages: short imperative summary (≤ 72 characters), optional body
  explaining why.
- **Commit author identity**: never change it. If it shows something
  unexpected (for example a private email instead of the GitHub noreply
  address), report it to the user.
- If something is committed by mistake: stop and tell the user. No history
  rewriting unless the user asks.

### Workflow

- Start every session by reading §0, the session log, and the open questions.
- New features: brainstorm with the user (questions one at a time, 2–3
  approaches with a recommendation) → short design spec → implementation plan
  → feature branch, one commit per step.
- `cargo build`, `cargo test`, `cargo clippy -- -D warnings` and
  `cargo fmt --check` must pass before every commit.
- Merge feature branches into `main` only when the user has tried the phase
  build and agrees.
- **CI** (GitHub Actions): build, test and clippy on Linux and Windows on
  every push. The user develops mainly on one OS; CI keeps the other one from
  breaking silently.
- **Releases**: when the user asks, tag `vX.Y.Z` on `main`; CI attaches the
  Linux binary and the portable Windows `.exe` to a GitHub release. Locally
  the user runs `cargo run --release -- file.pdf`.
- Unit-test pure logic in `marg-core`. Round-trip-test annotations in
  `marg-pdf` (create → save → reopen → verify). Check behaviour and speed
  manually on a release build.
- **Never benchmark or judge feel on debug builds.** Performance numbers come
  from release builds only.
- **Decisions about feel and taste belong to the user.** After changing how
  something feels or looks (hint layout, scroll steps, colors), hand over a
  build to try instead of judging it yourself.
- Create a `CLAUDE.md` in Phase 0 summarising these rules and pointing here.

### When Things Go Wrong

Long unsuccessful attempts waste the user's usage limit and bury the session
in noise. Prefer stopping early with a clear report.

- **Three attempts, then stop.** If the same problem survives three distinct
  fix attempts, stop. Report: what the symptom is, what was tried and why each
  attempt failed, what you now suspect, and two or three options. Wait for the
  user.
- **Diagnose before fixing.** Reproduce the problem and find the cause
  (logging, a minimal test, the debug overlay) before changing code. Do not
  stack speculative changes; revert a failed attempt before trying the next.
- **Never make a check pass by weakening it.** Do not delete, skip, or loosen
  tests, clippy lints, or type checks to get past an error, and do not hide
  errors with `unwrap_or_default`, empty matches or `#[allow]`. If a test is
  wrong, say so and ask.
- **Problems you cannot observe** (how something feels or looks, speed on the
  user's machine, a bug only the user sees): do not iterate on guesses. Ask
  for exact steps and what they saw, or add a way to display the relevant
  values (debug overlay, timing log).
- **MuPDF build problems** (especially on Windows): report the exact error and
  the toolchain versions before trying workarounds; do not vendor or patch
  MuPDF without asking.
- **Never risk the user's PDFs.** Test saving only on copies or fixtures.
  If a bug may have corrupted a file, say so at once and point to the backup.
- **Unexpected state** (unknown files, uncommitted changes you did not make,
  history that does not match the log): stop and ask before changing anything.
- **Stay in scope.** Do not refactor or "improve" unrelated code while fixing
  something. Note the idea in the open questions instead.
- **Write it down.** Record each failed approach and its cause in the session
  log, and unresolved problems in the open questions, so a later session does
  not repeat them.
- **Keep sessions short.** At a phase boundary, or when the context is filling
  up, update this document and suggest that the user start a fresh session.

---

## 11. Build Phases

Each phase ends with something the user can try, and a short check by them.

| Phase | Content | Done when |
|---|---|---|
| **0. Setup + spike** | `.gitignore`, `CLAUDE.md`, Cargo workspace, CI on Linux + Windows. **Spike**: with the `mupdf` crate, open a PDF, render page 1 into an eframe window, add one Highlight annotation by code, save incrementally, reopen. Confirm the API covers: rendering with annotations, structured text with char boxes, creating Highlight/Underline/Text/Square, setting color/opacity/QuadPoints/Contents/Subj, deleting, incremental save, document ID, outline, links, page labels. | The window shows page 1 and `:version`-equivalent text; the highlight appears in Okular or a browser; CI is green on both OSes; a short written report of API gaps. **If gaps exist, stop and discuss the stack with the user.** |
| **1. Viewer** | Continuous layout, rendering thread, cache and pre-render, scrolling, page navigation, counts, zoom and fit modes, command registry, TOML keymap with remapping, status bar, command line (`:q`, `:e`), state file (reopen at last position), recent files, debug overlay, startup timing. | The user reads a paper and a book with it and checks the speed targets in the debug overlay. |
| **2. Select & highlight** — **decision milestone** | Structured text model, two-stage hints (`f`, `F`), Caret and Visual modes with all motions, mouse selection, `y` copy, highlights `1`–`5` and underline `U`, provisional overlays, incremental save, autosave, backups, external-change detection, undo/redo. | **The user reads real papers for about a week and decides whether keyboard selection is fast enough and whether to continue.** |
| **3. Notes & boxes** | Comment popup (`c`, `e`), notes (`i`), box mode with grid hints, adjust and snap, annotation selection (`]a`, `[a`, `ga`, click), edit/delete/recolor/toggle, tooltips, annotation list panel, reading categories from foreign annotations. | The user annotates a full paper (highlights, comments, boxes) and checks them in another viewer and via Zotero's "Import annotations". |
| **4. Reading features** | Single and spread layouts with the defaults rule, outline overlay, search, marks, jump history, link hints and link following, help overlay. | The user reads a slide deck and a book chapter with it. |
| **5. Export & release** | Markdown export, `:config` default file, config auto-reload and error reporting, password prompt, read-only handling, tagged release builds (Linux binary + portable Windows `.exe`), polish from the user's list. | `v1.0.0` is tagged and the user replaces Zathura/Zotero's reader with marg. |

The early phases are the prototype. If keyboard selection does not feel right
at the decision milestone, change the design (hint scheme, motions) before
building more.

---

## 12. Open Questions & Known Risks

**Open decisions (user)**

- Category names and colors in §2.8 are suggestions; confirm or change.
- **Keyboard layout**: the default keymap is US/vim-style. On a German QWERTZ
  layout, `[ ] { } /` need AltGr or Shift. Decide whether to keep these
  defaults (and remap personally) or ship a QWERTZ-friendly default profile.
- Two-stage hints are a design proposal; the `words` mode exists as an
  alternative. Confirm at the decision milestone.
- `spread_first_page_alone` default (`false`).
- Exact grid size and step sizes in box mode.
- **Portable Windows `.exe` (Phase 5)**: MuPDF's Windows build (via
  `mupdf-sys` → `mupdf.sln`) uses the dynamic CRT, so static CRT is off and
  the `.exe` needs the MSVC runtime. `CFLAGS` won't change this (MSBuild, not
  `cc`). Options: patch `RuntimeLibrary` to `MultiThreaded` in MuPDF's
  vcxproj files (patching MuPDF: ask the user first), or ship with the
  runtime requirement / app-local runtime DLLs.

**Risks**

- ~~**`mupdf` crate API coverage**~~ Checked in Phase 0: no blocking gaps
  (§9 spike report). Two small ones are covered by raw object access.
- **Building MuPDF on Windows** (toolchain, clang/bindgen requirements).
  Checked in Phase 0 with CI on Windows.
- **Hint/caret selection not feeling faster than a mouse.** This is the core
  bet. Checked at the decision milestone with a week of real use.
- **Reading order in multi-column papers** (caret motions, selection across
  columns). Checked in Phase 2 with the two-column fixture and the user's
  papers.
- **Text input in egui** (German umlauts, dead keys, IME on Windows) for the
  comment editor. Checked in Phase 3 on both OSes.
- **Incremental save edge cases** (repaired files, encrypted files, Windows
  file locks while Zotero has the file open). Mitigated by backups and the
  rules in §2.9; round-trip tests in Phase 2.
- **Rendering throughput with one document thread.** Measured in Phase 1;
  extra render workers only if needed.

**Known issues**

- Spike window renders page 1 at a fixed 2× scale and does not track the
  window's pixel ratio or zoom; replaced in Phase 1.
- First CI run on Windows is slow (MuPDF compiled from source, no cache);
  later runs use `rust-cache`.
- Windows **debug** builds warn `LNK4098: defaultlib 'msvcrt' conflicts`:
  `mupdf-sys` links MuPDF's Debug config with the debug CRT (`msvcrtd`)
  while Rust always uses the release CRT. Inherent to `mupdf-sys`; release
  builds should be clean (not yet checked in CI; check in Phase 5 release
  builds).

---

## 13. Ideas for Later (not planned)

- Dark mode / page recoloring (Zathura-style recolor).
- Reading Zotero's own annotations, or syncing with Zotero.
- Exporting boxes as cropped PNG images into the Markdown export.
- Copy a highlight as a citation-style quote with page number.
- Search across the annotations of all previously opened files.
- Tabs or split view (paper + supplement side by side).
- Freehand / pen annotations.
- Presentation mode (fullscreen, no status bar) for slide decks.

---

## 14. Session Log

One entry per working session, newest first. Keep entries short. This is the
handover to the next session, which has no memory of this one.

Format:

```
### YYYY-MM-DD — <phase and topic>
- Done: what was built or changed (with version or branch).
- Decisions: what the user decided, and why.
- Tried and failed: approaches that did not work, and the cause.
- State: does it build, do tests pass, what is deployed.
- Next: the exact next step.
- Watch out: anything a new session would otherwise get wrong.
```

### 2026-10-27 — Phase 0: setup + MuPDF spike
- Done (branch `phase-0-setup`, merged into `main` after the user's check): Cargo workspace with
  `marg-core`, `marg-pdf`, `marg`; `CLAUDE.md`; CI (fmt, build, test, clippy
  on Linux + Windows); `.gitattributes` (`*.pdf binary`);
  `tests/fixtures/gen_fixtures.py` → `text.pdf` (2 pages, outline, internal +
  URI link, page labels i/1, title, fixed `/ID`); spike tests
  (`crates/marg-pdf/tests/spike.rs`); `highlight_copy` example; spike window
  (page 1 rendered on a worker thread, status bar with version + git hash).
- Decisions: static CRT on Windows dropped (user left the choice to Claude):
  MuPDF's Windows build uses the dynamic CRT; portable `.exe` revisited in
  Phase 5.
  `mupdf` 0.8.0 with default features off except `base14-fonts`
  (no JS, XPS, EPUB, tesseract etc.; smaller and faster to build); eframe
  0.37 (wgpu default).
- Tried and failed: first Windows CI run failed the round-trip test because
  git converted `text.pdf` to CRLF on checkout (no NUL bytes → treated as
  text), MuPDF repaired it, and incremental save was refused. Fixed with
  `.gitattributes`. `+crt-static` gave linker warning LNK4098; removed.
  First guess (MuPDF uses static CRT) was wrong: a debug-only LNK4098
  remains because `mupdf-sys` links the debug CRT in Debug config.
- State: builds; 5 tests pass; clippy and fmt clean; CI green on Linux and
  Windows. User ran the spike window on Linux/Wayland: page 1 shows fine.
  User confirmed the spike highlight on their own paper (copy) in another
  viewer.
- Next: Phase 1 (viewer), after the user's go-ahead. Phase 1 first
  step: a real document thread in `marg-pdf` with request/result channels.
- Watch out: eframe 0.37 uses `App::ui(&mut self, ui, frame)` and
  `egui::Panel::bottom` (not `update` / `TopBottomPanel`). MuPDF objects
  must never cross threads. Real PDFs contain whitespace-only stext lines
  (Grudin 2008 paper: first line on page 1 is " "); hints, caret motions and
  selection must skip them.

### 2026-10-27 — Planning
- Done: this concept document, from a brainstorming session. No code yet.
- Decisions: standalone reader (no Zotero integration); annotations stored
  inside the PDF as standard annotations with incremental saves; types:
  highlight, underline, comments/notes, boxes (no freehand); modal vim-like
  keymap, fully remappable; selection via hints plus caret/visual mode; mouse
  works as fallback; continuous, single and spread layouts; one window per
  file; inline comment editor; Markdown export grouped by named color
  categories; portable Windows `.exe`; private GitHub repo; Linux + Windows.
  Stack: Rust + MuPDF (`mupdf` crate) + egui (recommended; not objected to).
- State: empty repository.
- Next: Phase 0 (setup + MuPDF spike).
- Watch out: all numbers marked *(start)* are untested starting values;
  never test saving on the user's real PDFs; the decision milestone is the end
  of Phase 2.

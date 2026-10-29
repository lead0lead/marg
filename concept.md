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
| **Status** | **v1.0.0 released** on 2026-10-29 (https://github.com/lead0lead/marg/releases/tag/v1.0.0): Linux binary and Windows `.exe`. Phases 0–5 merged into `main`. |
| **Current phase** | none: all planned phases are done; the user uses marg daily |
| **Next step** | The user uses marg as their main reader for a few weeks and collects a polish list; then plan the fixes with them (feature branch, release as 1.0.x / 1.1.0). The Phase 3–5 choices in §12 are open. |
| **Decision milestone** | Passed 2026-10-28: after reading real papers with the Phase 2 build, the user said selection works and asked to move on to the next phase ("I feel like we are ready to move on"). This was before the planned week ended; the user's call. |
| **Live version** | v1.0.0 |
| **Repository** | private GitHub repository `marg`, branch `main` |
| **App** | runs locally: `cargo run --release -- [<file.pdf> [--page N]]` (continuous, single and spread layouts; outline, search, marks, jump history, links, help; hints, caret/visual selection; highlights, underlines, comments, notes and boxes saved into the PDF; annotation list; no file → recent files list) |
| **Target** | Linux (X11 and Wayland) and Windows 10/11, desktop only. Out of scope: macOS, mobile, tablets, pen input. |
| **Last updated** | 2026-10-29 (v1.0.0 released) |

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
- `:close` (alias `:bd`, key `Ctrl-w`) saves pending changes, closes the
  file and shows the recent files list (added 2026-10-28 at the user's
  request; nvim has no built-in start page, its dashboards are plugins,
  and closing the last buffer with `:bd` often brings one back). Like `q`,
  it does not close if saving fails.
- Password-protected PDFs: prompt for the password in the command line. PDFs
  whose permissions forbid annotation open read-only, with a status message.
- Files that fail to open: show the MuPDF error in the status bar and in the
  log. Never crash.

**As implemented (Phase 5, passwords):** `Request::Open { path, password
}`; `Doc::open_with_password` answers `OpenError::NeedsPassword` or
`WrongPassword` (other failures `Failed`). The UI then shows the prompt
"Password for <name>: ••••" in the status bar (mode `Password`; the
typed text is never shown, kept in history or logged); Enter tries it
("Wrong password for <name>" and the prompt again), Esc gives up ("Not
opened"; back to the current file, or the recent files list). The
password that worked is kept in memory for the session (`OpenDoc`, and
in `Doc` for reopening after a full save), so `:e!` and reloads after
outside changes do not ask again; it is never written anywhere. Saving
keeps the encryption: incremental saves append encrypted objects and
full saves use MuPDF's default "keep" (tests in
`marg-pdf/tests/password.rs` on a file MuPDF encrypts with AES-256: no
clear text of a comment in the file, still needs the password). The
`mupdf` crate cannot tell whether the user or the owner password
worked. Opening that AES-256 file takes ~165 ms instead of ~6 ms
(MuPDF's key derivation; release, Xvfb).

**As implemented (Phase 5, read-only):** decided by the user
2026-10-29: obey the permissions, with an override for all files and
one for the open file. `DocInfo.can_annotate` is bit 6 of `/P` in the
encryption dictionary (read directly: the `mupdf` crate's
`permissions()` falls back to "everything" because `/P` always has
undefined high bits; checked, it gives 3900 = all for a file that
forbids annotations); `DocInfo.writable` is false when opening the file
for appending is refused (a lock by another program does not count).
Read-only files show "Read-only: the PDF forbids annotations; :unlock"
(or "… no permission to write this file") on opening and when an edit
is tried, and `[RO]` after the file name. Refused: every command with
`Command::edits()` (highlights, underline, category prompt, comment,
note, box mode, delete, recolor, type switch, comment edit, erase,
undo, redo) and `e`/`x` in the annotation list; reading, copying and
exporting work. `[annotation] ignore_permissions = true` annotates such
files anyway (applies on config reload); `:unlock` (`file.unlock`) does
it for the open file until it is closed (a reload keeps it; not saved
in the state file). Neither helps a file marg may not write.

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

**As implemented (Phase 1):** `marg-pdf/src/thread.rs` owns the document.
The UI sends the full list of renders it is missing each time that list
changes (`Request::Render`), most urgent first: visible pages, then ±2
pages; jobs not yet started are dropped when a new list arrives. Keys still
in transit to the UI are not rendered twice. A render key is page + scale
(thousandths of a device pixel per point) + optional 512 px tile. Textures
live in a frame-aware LRU (`marg-core/src/lru.rs`, 256 MB): anything drawn in
the current frame is never evicted. Above 200% zoom, visible tiles plus a
one-tile margin are rendered over a low-resolution full page (at 200%).
While a render is missing, the cached full page with the closest scale is
drawn stretched. A failed `:e` keeps the current document.

**As implemented (Phase 2):** the UI requests the text of visible pages
(and of the pages next to the caret) with `Request::Text`; text requests
are served before renders. Extraction uses no MuPDF flags (no
`DEHYPHENATE`, which would merge lines); boxes are page-relative points.
Measured on 53 real PDFs (release): page text in 0.7 ms median, 3.8 ms p90,
35 ms max. Annotation edits (`Request::Edit`) are served first; each
successful edit raises the page's revision, which is part of the render
key, so the page re-renders; older renders of the page stay as a fallback
until the new full page arrives and are then dropped. Until then the UI
draws the new annotation itself (`select::draw_provisional`).

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

**As implemented (Phase 1):** Continuous only (`marg-core/src/view.rs`), in
view units = points × zoom (logical pixels). Pages are centred; no margin
above the first page. Fit-width uses the widest page, fit-page the largest
page. A landscape first page opens in fit-page (Single comes in Phase 4).
The "current page" (status bar, `J`/`K`) is the page at the viewport's
vertical centre. `zoom.reset` (`=`) keeps the top centre in place; other
zooms keep the viewport centre, Ctrl+wheel keeps the point under the mouse.

**As implemented (Phase 4):** `marg-core/src/view.rs` lays pages out in
rows: one page per row, two in Spread (`spread_first_page_alone` puts the
first page in a row of its own; an odd last page is alone). Continuous
stacks all rows; Single and Spread show one row, centred when it is
smaller than the window, and scrolling stops at its edges (`j`/`k`,
`Ctrl-d`/`Ctrl-u`, the wheel: no spill-over). Fit width fits the widest
row (two pages and the gap in Spread), fit page the largest row. `J`/`K`
move by row. **Space / Shift-Space** (`scroll.screen_down|up`, user's
choice 2026-10-29, "matching Zathura"): in Single/Spread, scroll one
window height, and at the bottom (top) of the row show the next row from
its top (the previous one from its bottom); in Continuous they stay next
/ previous page (Claude's reading of the choice; see §12). `Ctrl-l`
(`view.layout_cycle`) cycles continuous → single → spread, `:layout
continuous|single|spread` sets one, `view.spread_first_alone` toggles the
cover option (no default key). Switching keeps the reading position (in
a spread, the position is its left page). The layout is saved per file;
a new file uses `default_layout_landscape` (fit page) when its first
page is landscape, else `default_layout_portrait` (fit width). The
status bar shows `12–13/34` for a spread. A caret moving to another page
in Single/Spread shows that page.

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
  indented by level. It works like every list (§3, "Lists"): `j/k` move, `/`
  filters (fuzzy), Enter jumps, Esc closes. If the PDF has no outline, show a
  status message.

**As implemented (Phase 4):** jump list in `marg-core/src/jumps.rs`
(vim-like: a new jump drops the forward history; the same position is
not stored twice in a row; `[view] jump_history = 100`), per open file,
not saved. Jumps: `gg`, `G`, `{n}G`, `:n`, Enter in search, `n`/`N`,
outline, marks and links. Positions are the window's top-left document
point. Marks: `m` / `'` wait for the next key; `a`–`z` set or go, any
other key cancels; the pending `m`/`'` shows at the right of the status
bar. Marks are saved in the state file at once (`marks` per file).
Outline: `o` (`outline`, mode `OUTLINE`) asks the document thread
(`Request::Outline`, read on first use and kept); the overlay covers the
left `ui.outline_width` (0.4) of the view, entries indented by level
with their page label on the right, the entry of the current page
selected. Entries without a destination are dim. Destinations
(`marg_core::outline::Dest`): page plus the PDF's left/top if given,
shown at the window's top left. Links: `Request::Links(page)` for the
visible pages; internal links jump, URIs with a scheme (`https:`,
`mailto:`) ask "Open … ? y: yes  any other key: no" (mode `Confirm`),
then start `xdg-open` (Windows: `rundll32 url.dll,FileProtocolHandler`,
no shell) without waiting. A click (press and release within 4 px, no
selection made) on a link follows it; the pointer is a hand over links.

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

**As implemented (Phase 4):** matching in `marg-core/src/search.rs` on
the core text model, so it behaves like copying: line and block breaks
are spaces, whitespace runs match any whitespace, a letter + `-` at a
line end followed by a lowercase letter joins the word (as `y` does),
ligatures (ﬀ ﬁ ﬂ ﬃ ﬄ ﬅ ﬆ) match their letters, soft hyphens are ignored;
hits do not overlap. The document thread (`Request::Search`) extracts
and searches one page per turn, after renders and urgent requests, from
the current page to the end, then from the start; pages with hits come
back as `Response::Searched`, then `SearchDone`. A new query replaces the
running search. Prompt `/` (mode `Search`, own history): the search runs
while typing and the view goes to the first hit at or after where `/`
was pressed (a third from the top if it was not in view); Esc goes back
there; Enter keeps the search and records the jump; Enter on an empty
prompt repeats the last query. Hits on visible pages are drawn in
`search_hit`, the current one in `search_current` (§6). `n`/`N` (with
counts, wrapping) go from the current hit if it is in view, else from
the view's top. The status bar part `search` shows `hit 3/27` (`+` while
searching, `-` before a hit is picked), "searching", "no hits". "Not
found: …" when a finished search has none. Esc in Normal mode (with no
annotation selected) hides the hits; `n` then searches the last query
again from the view. Cost: one text extraction per page and search (§2.2
measured 0.7 ms median), streamed; no cache on the document thread.

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

**As implemented (Phase 2):** labels come from `marg-core/src/hints.rs`
(breadth-first, prefix-free, shortest first; earlier targets get shorter
labels). Line labels sit left of the line; word labels sit on the word's
start (they cover its first letters). Stage 2 uses `chars` + `extra_chars`
(`[hints]`); a line with one word is picked at once. A word is a
whitespace-separated run (punctuation included). Characters that match no
label are ignored; Backspace removes one; Esc cancels and restores the
previous selection. While the second word of `F` is picked, the first is
shown selected. The typed prefix appears at the right of the status bar.
If the visible text is still loading, the message is "Text is still
loading".

**As implemented (Phase 3):** `ga` (`hint.annot`) labels every visible
annotation (those whose boxes are on screen, in reading order) with the
`[hints] chars` labels, on the top-left corner of its first box; picking
one selects it (and leaves Caret/Visual). "Still reading the
annotations" while a visible page is not listed yet, "No annotations in
view" if there are none.

**As implemented (Phase 4):** `gl` (`hint.links`) labels the visible
links on their top-left corner (like `ga`), in reading order; picking
one follows it. "Still reading the links" / "No links in view".

### 2.7 Caret and visual selection

- **Caret mode** shows a block cursor on a character (§6). Motions
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

**As implemented (Phase 2):** `marg-core/src/caret.rs`. The caret sits on
a character and is drawn at its left edge. Words are whitespace-separated
(vim's `W`, so `w`/`e` include punctuation). Added: `h`/`l` (and
Left/Right) move by one character, to adjust a selection end. Sentence
starts: the first word of a block, or a word after `.`/`?`/`!` (up to two
closing quotes or brackets in between). `{` goes to the start of the
current block, or the previous one when already there; `}` to the next
block's start. Motions cross pages; a page whose text is not loaded yet
stops them (it is requested, the next key works). Visual: `v`/`V` in
Visual switch the kind, or end the selection (back to Caret) when it is
already that kind. `y` copies and stays in Visual ("Copied N characters").
Copy text: lines joined with a space; a line ending in a letter + `-`
followed by a lowercase letter is joined without the hyphen (so
"well-known" split at the hyphen becomes "wellknown"). Mouse: a single
click clears the selection; double/triple click within 400 ms
(`ui.multi_click_ms`). Dragging into the 16 px edge zone at the top or
bottom (or past it) scrolls, faster the further in (`ui.drag_scroll_rate`
px/s per px, `ui.drag_scroll_edge_px`), and extends the selection.

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

**As implemented (Phase 2):** highlights (`1`–`5`) and underlines (`U`,
last-used category, default 1) from a selection, then back to Normal. One
annotation per page the selection touches, one QuadPoint per selected
line part (leading and trailing spaces left out). `/Subj` = category name,
`/T` = `annotation.author` or the OS user name (`USER`/`USERNAME`), opacity
only on highlights. Undo/redo works per action (all pages of it), for
highlights and underlines; the stack is cleared when a file is opened or
reloaded. Categories that are not `#RRGGBB` give an error message. Reading
categories from foreign annotations comes in Phase 3.

**As implemented (Phase 2, pulled forward from Phase 3 at the user's
request):** `]a` / `[a` (with counts) select the next or previous
annotation in reading order (page, top, left), from the selected one or
from the top of the view, and scroll it into view; a click selects the
topmost annotation under the pointer (its quads, or its rectangle). The
selection is a dashed outline in the annotation's color with a dark dashed
line under it (so light colors show on white), and the status bar shows
type, page, `/Subj` and the start of `/Contents`. `x` deletes it; Esc or a
click elsewhere deselects. The document thread lists every page's
annotations when idle after opening (edited pages right away);
`]a` before that says "Still reading the annotations". Deleting only
unlinks the annotation (and its popup) from the page's `/Annots`; `u`
puts the same object back, so nothing is lost, also for annotations from
other programs. Links, form fields and popups are not listed. Still in
Phase 3: `ga`, `e`, recolor, `U` toggle, tooltips.

**Erasing (Phase 2, added 2026-10-28 at the user's request, "option B"):**
Visual `d` (`annotate.erase`) removes highlights and underlines from the
selected text only (`marg-core` `annot::erase`). A character belongs to an
annotation if its centre is inside one of its boxes. Each affected
annotation is deleted; the parts outside the selection become new
annotations of the same type, color, opacity, `/Subj` and author, one per
remaining stretch in reading order (so erasing the middle of a highlight
leaves two; Zotero imports them as two). The comment stays with the first
part; other fields of foreign annotations (dates, `/NM`, popups) are not
copied to the parts. One undo step: undo deletes the parts and puts the
original objects back. Squiggly and strike-out annotations are left alone.
Option A (remove whole highlights touching the selection) is Visual `D`
(`annotate.erase_whole`), added as a second key at the user's request; by
analogy with vim, where Visual `D` deletes whole lines.

**As implemented (Phase 3):**

- **Categories of foreign annotations** (`annot::category_of`): `/Subj`
  matching a category name (ignoring case and surrounding spaces), else
  the category with the nearest color (squared RGB distance; Zotero's
  colors match the defaults exactly), else Uncategorised (no color). The
  status bar of a selected annotation shows the category, not `/Subj`;
  subtypes show as Note (`Text`) and Box (`Square`).
- **Selected annotation keys**: a keymap layer `[keys.annot]` on top of
  Normal, active while an annotation is selected: `1`–`5`
  `annot.recolor(n)` (color and `/Subj`), `U` `annot.toggle_type`
  (highlight ↔ underline; opacity becomes 1, or `highlight_opacity` for a
  highlight), `e` `annot.edit`. `x` stays in Normal. Without a selection
  digits are counts as before.
- **Edits in place** (`Edit::Update`, `Doc::update_annotation`): recolor,
  type switch and comments change the existing object, so `/NM`, dates
  and popups survive (Zotero sees the same annotation). The type switch
  rewrites `/Subtype` and drops `/AP`; MuPDF draws a new appearance
  (checked by rendering). Each is one undo step (`UndoOp.changed`, old
  values from the listing).
- **Undo/redo of created annotations**: undo deletes (unlinks), redo now
  puts the *same object* back (`Edit::Restore`) instead of creating a new
  one, so later steps that name its object number keep working.
- **Comment editor** (`marg/src/comment.rs`, Insert mode, `INSERT`):
  a popup right of the annotation (left if there is no room, else over
  it), `ui.comment_width_px` (360) wide, up to `ui.comment_max_height`
  (0.4) of the window, then scrolling; monospace; egui `TextEdit`, which
  always keeps the focus. Enter adds a line, `Ctrl-Enter` saves (trailing
  whitespace trimmed; unchanged text saves nothing), Esc cancels, and on
  changed text the first Esc shows "Esc again to discard the changes".
  The status bar shows the editor's hint instead of info messages.
  Umlauts and ß typed directly work (Xvfb); dead keys could not be tested
  there (see §12).
- **Visual `c`** (`annotate.comment`): highlight in the last-used category,
  then the editor on its first part. The comment is part of the
  highlight's undo step (redo restores the object with it); Esc keeps the
  highlight. If the creation is not confirmed yet when saving, the comment
  is applied when it is (`AfterCreate`).
- **Notes** (`i`, `annotate.note`, Normal/Caret/Visual): a PDF `Text`
  annotation in the last-used category, icon `annotation.note_size_pt`
  (20) square, its bottom-left corner at the caret character's top-left;
  without a caret at the top left of the visible part of the current page
  (half an icon in from the edges). The editor opens at once; leaving it
  without text (Esc, or saving empty) removes the note and its undo step.
- **Tooltips**: the comment of the annotation under the mouse, else of the
  selected one, in a panel like the editor's (wrapped, cut off at the
  same height); not in Insert, Hint, Box or list modes.
- **Box mode** (`b`, `annotate.box`, `BOX`; `marg/src/boxmode.rs`,
  geometry in `marg-core/src/boxsel.rs`): a `box_grid_cols` × 
  `box_grid_rows` (9 × 12) grid over the whole view (user's choice
  2026-10-28), labeled with `[hints] chars` (two or three characters). The
  first label picks a cell, the second another; the box spans both cells,
  in either order, on the page under the first cell's top-left corner,
  clamped to that page. Backspace removes a typed character, or goes back
  to the first cell. Then: `hjkl` move the bottom-right corner, `HJKL`
  the top-left one, by `box_step_pt` (4), with Ctrl by `box_fine_step_pt`
  (1) (`Ctrl-Shift-hjkl` for the top-left corner); corners stop at the page
  edge and at a 2 pt minimum size. `1`–`5` set the category (shown in the
  status bar), `s` snaps, Enter saves (a `Square` with
  `box_stroke_pt` line width, no fill, category color and `/Subj`), Esc
  cancels. A mouse drag in Box mode draws the box (on the page where the
  drag starts) and goes straight to adjusting. Box mode keys are fixed,
  not in the keymap (like the list keys).
- **Snap** (`s`): the image the box overlaps most (by area), from
  `Request::Images` (MuPDF text page with `PRESERVE_IMAGES`, image blocks
  only), asked for on the first `s` per page and cached. Not part of the
  page text: with images kept, a Springer book cover's text took 394 ms
  instead of < 80 ms, which would delay the first render (text is
  requested before renders). "No image under the box" if none overlaps.

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

**As implemented (Phase 2):** `marg-pdf/src/document.rs` (`Doc::save`,
`ensure_backup`, `poll_disk`), driven by the app. The backup is made when
the first edit of a session arrives (before the file is ever written); if
it cannot be made, the edit fails ("cannot back up the file, so it was not
changed"). Backup name: hex of `/ID[0]`, else `path` + an FNV-1a hash of
the path, then `-<filename>`. A save appends (incremental) when MuPDF
allows it and the file is unchanged on disk; otherwise (or with `:w!`) the
whole file goes to `.<name>.<pid>-<n>.marg-tmp` next to it and is renamed
over it (on failure, e.g. Windows, the document is closed and the rename
retried), and marg continues from the new file. A file changed on disk is
never saved over without `:w!`; while that conflict lasts, autosave waits.
Autosave runs `annotation.autosave_secs` after the last confirmed edit;
after a failed save it retries after the same delay. `q`, closing the
window and `:e` save first and do not go on if the save fails (the message
says so; `:q!` closes anyway). `on_exit` saves once more as a last resort.
The file check runs on the document thread every `ui.file_check_secs`
(1 s) when the app asks (`Request::CheckDisk`); a clean document reloads
("Reloaded from disk") and keeps its position; an unsaved one shows "File
changed on disk; :w! keeps marg's version, :e! reloads".

### 2.10 Annotation list

- `A` opens a side panel (35% of the window width *(start)*) listing all
  annotations in page order: color swatch, category, page number, the first
  80 characters of the highlighted text (or "[Box]" / "[Note]"), and the
  comment.
- Typing filters by text and comment (fuzzy). `1`–`5` filter by category;
  `0` clears the filter.
- `j/k` move, Enter jumps to the annotation and selects it, `e` edits the
  comment, `x` deletes. Esc or `A` closes the panel.

**As implemented (Phase 3):** `A` (`annot.list`), mode `ANNOTATIONS`;
rows from `marg-core/src/annotlist.rs`, panel in `marg/src/annotlist.rs`.
The panel is drawn **over** the right `ui.annot_list_width` (0.35) of the
pages, not beside them, so the view keeps its size and pages are not
re-fitted and re-rendered when it opens (Claude's choice; the user may
prefer a real side panel). Each row: color swatch, `p. <label>`, category,
the marked text (characters whose centre lies in the annotation's boxes,
lines joined like `y`; first 80 characters), or `[Box]`, `[Note]`,
`[<Subtype>]`; the comment in italics. The list keys of §3 "Lists"
(`/` fuzzy filter over text and comment, Esc clears it, then closes),
plus `1`–`5` (one category, shown as `[Name]` in the header), `0` (all),
`e` (go to it and edit; closes the list), `x` (delete; the list stays),
`A` closes. Enter goes to the annotation (a third from the top if it was
off screen) and selects it. Opening the list asks for the text of
annotated pages; rows update as annotations and texts arrive.

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

**As implemented (Phase 5):** `:export md [path]` (`Command::Export`,
`file.export`); text in `marg-core/src/export.rs` from the annotation
list's rows (`annotlist::entries`), so the marked text is the same as
there and in `y`. The path: `<stem>.notes.md` next to the PDF; a given
path (`~` expanded, relative to the working directory like `:e`), or
that name inside it if it is a folder. Before writing, every page's
annotations are listed again (idle queue; edits are applied as they
arrive, so this includes every edit sent before) and the text of the
annotated pages is read; "Exporting…" until then, then "Exported N
annotations to <name>" (a warning if pages could not be read). Title
and section names are put on one line; quoted text is joined into one
line; comments are written as typed (trimmed). Highlights, underlines,
squiggly and strike-out are quotes; boxes, notes and other types are
**[Box]**, **[Note]**, **[<Subtype>]**. The date is UTC. Written on the
UI thread (small file).

### 2.12 Help overlay

`?` shows the bindings active in the current mode, generated from the live
keymap (so remaps show up). Esc closes.

Added 2026-10-28 at the user's request (Phases 4 and 5):

- **Help page** (Phase 4): `:help` opens a full-window, scrollable page
  with every mode's bindings grouped by topic (moving, selecting,
  annotating, files), the `:` commands, and a short explanation of hints
  and categories. Typing filters it, like the other lists. Generated from
  the live keymap and the command registry's help texts, so it is never out
  of date and shows remaps.
- **Cheat sheet** (Phase 5): a compact one-page summary of the default
  keys, in the README (which does not exist yet; it comes with the release)
  and in `docs/cheatsheet.md`. Generated from the default keymap by a small
  tool or test, and a test fails when the committed cheat sheet no longer
  matches the defaults.

**As implemented (Phase 4):** `marg-core/src/help.rs` builds the lines
from the live keymaps and the registry's help texts; `marg/src/help.rs`
draws them (mode `HELP`). `?` (`help`): the keys of the current keymap
(Normal, Caret, Visual, or with an annotation selected), by topic
(Moving, Selecting, Annotating, Files), one line per command with all
its keys, in a centred panel. `:help` / `:h` (`help.page`): the whole
window: Normal by topic, then what Caret, Visual and the selected
annotation add, then fixed keys (hints, box mode, comment editor, lists,
search prompt), `:` commands, the categories from the config, and every
command name. Both use the list keys; `/` filters (every word of the
query must appear in a line, ignoring case: fuzzy matching matched most
long lines), keeping each match's heading. Esc (or `?` outside the
filter) closes.

**As implemented (Phase 5, cheat sheet):** `help::cheatsheet(level)`
renders the help page of the default config (without the list of
command names) as Markdown tables, with `1`–`5` rows merged ("1-5").
`docs/cheatsheet.md` and the "Keys" section of `README.md` (between
the `<!-- cheat sheet … -->` markers) hold it; the test
`committed_cheat_sheets_match_the_defaults` fails when either differs,
and `MARG_BLESS=1 cargo test -p marg-core cheat` rewrites both. The
README covers install, use (with Zotero), saving, config paths and
licence; it says the Windows `.exe` needs the MSVC runtime (to change
if step 7 links it statically).

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
| | `Ctrl-w` | save and close the file, show recent files |
| Caret | `w b e ge 0 $ j k ( ) { }` | motions |
| | `v` / `V` | start visual / line-visual |
| Visual (selection) | motions | extend selection |
| | `o` | swap selection end |
| | `1`–`5` | highlight with category |
| | `U` | underline |
| | `Enter`, then `1`–`5` or `Enter` | highlight via category prompt (last category on `Enter`) |
| | `c` | highlight + comment |
| | `y` | copy text |
| | `d` | erase highlights/underlines from the selected text |
| | `D` | delete every highlight/underline the selection touches, whole |
| Selected annotation | `e`, `x`, `1`–`5`, `U` | edit comment, delete, recolor, toggle type |
| Command | `:w`, `:w!`, `:q`, `:q!`, `:e path`, `:e!`, `:close` / `:bd`, `:export md [path]`, `:layout …`, `:config`, `:unlock`, `:set name=value`, `:debug` | |

Note: `L` is deliberately unbound in Normal mode (it is used in Box mode for
corner moves); layout cycling uses `Ctrl-l`.

**As implemented (Phase 1):** Normal-mode bindings for scrolling, pages,
zoom, fit, `:` and `q` (`marg-core/src/keymap.rs`, `DEFAULT_NORMAL`), plus
arrow keys (scroll), `PageDown`/`PageUp` (page), `Home`/`End` (first/last
page). Key notation: one character (`j`, `G`), a named key (`Space`,
`PageDown`, `Esc`, …), modifier prefixes (`Ctrl-o`, `Shift-Space`),
character sequences (`gg`, `]a`) or space-separated keys (`g Ctrl-o`).
`"nop"` removes a default binding. Command names: `scroll.down|up|left|right`,
`scroll.half_down|half_up`, `page.next|prev|first|last`,
`zoom.in|out|reset|fit_width|fit_page`, `cmdline`, `quit`, `quit.force`,
`version`, `debug.toggle`. A digit is a count unless a binding starts with
it. `:` commands: `q`, `q!`, `e <path>` (Tab completes directories and PDFs),
`version`, `debug`, `<n>` (go to page n), or any command name
(Phase 5 adds `export md [path]` and `config`). Up/Down
browse the session's command history. Windows AltGr (Ctrl+Alt) characters
are taken from the key event, since egui drops text while Ctrl is held.

**Lists** (recent files; outline and annotation list later), decided by
the user 2026-10-28 (to re-evaluate after real use): a list opens for
moving. `j`/`k`, arrows, `Ctrl-n`/`Ctrl-p` move (stopping at the ends),
`gg`/`G` (and `Home`/`End`) go to the first/last entry, `Ctrl-d`/`Ctrl-u`
(and `PageDown`/`PageUp`) move half the visible rows, Enter opens, Esc
clears the filter or closes the list. `/` starts filtering: typing narrows
the list live with the best match selected, `Ctrl-n`/`Ctrl-p` and arrows
move, Enter opens the selection directly (`/kahn⏎`), Esc stops typing but
keeps the filter (move through the results with `j`/`k`), Backspace on an
empty filter stops filtering, `/` again continues the filter. Rejected: `:`
for filtering (`:` stays for commands) and opening lists in filter mode
(telescope-style; conflicts with single-key actions in the annotation list
and costs an extra Esc). **As implemented:** `marg-core/src/list.rs`
(`ListNav`), used by `marg/src/recent.rs`; keys are fixed (not in the
keymap yet). The prompt line shows `/filter▏` while filtering, the filter
dimmed after Esc, or a key hint when empty.

**As implemented (Phase 2):** Caret and Visual mode have their own
tables, `[keys.caret]` and `[keys.visual]` (`marg-core/src/keymap.rs`,
`DEFAULT_CARET`, `DEFAULT_VISUAL`). The Caret keymap is the Normal one with
the Caret bindings on top; Visual adds its bindings on top of Caret. So
Normal keys like `J`, `Ctrl-d`, `+` still work in Caret and Visual mode.
New Normal keys: `f`, `F`, `u`, `Ctrl-r`. New commands: `hint.caret`,
`hint.select`, `caret.char_next|char_prev|word_next|word_prev|word_end|
word_end_prev|line_start|line_end|line_down|line_up|sentence_prev|
sentence_next|block_prev|block_next`, `visual.char`, `visual.line`,
`visual.swap`, `annotate.highlight(1)`…`(5)`, `annotate.underline`,
`annot.next` (`]a`), `annot.prev` (`[a`), `annot.delete` (`x`),
`annotate.pick` (Visual `Enter`: the status bar lists the categories, a
digit highlights, `Enter` uses the last category, Esc goes back),
`selection.copy`, `undo`, `redo`, `file.save` (`:w`), `file.save_force`
(`:w!`), `file.reload` (`:e!`), `file.close` (`:close`, `:bd`, `Ctrl-w`). Esc (leave Caret/Visual) is fixed, not a
binding. Since `1`–`5` are bound in Visual mode, counts there can only use
other digits. The status bar shows `HINT`, `CARET`, `VISUAL` and ● for
unsaved changes; `:` returns to the mode it came from.

**As implemented (Phase 3):** new Normal keys `ga` (`hint.annot`), `i`
(`annotate.note`), `b` (`annotate.box`), `A` (`annot.list`); Visual `c`
(`annotate.comment`); new table `[keys.annot]` (Normal while an
annotation is selected): `1`–`5` `annot.recolor(1)`…`(5)`, `U`
`annot.toggle_type`, `e` `annot.edit`. Insert, Box and the annotation list
have fixed keys (§2.8, §2.10). New modes in the status bar: `INSERT`,
`BOX`, `ANNOTATIONS`.

**As implemented (Phase 3b):** Normal `gr` (`view.recolor`) toggles dark
pages.

**As implemented (Phase 4):** new Normal keys `Ctrl-l`
(`view.layout_cycle`), `Ctrl-o` / `Ctrl-i` (`jump.back|forward`), `m` /
`'` (`mark.set|jump`), `o` (`outline`), `/` (`search`), `n` / `N`
(`search.next|prev`), `gl` (`hint.links`), `?` (`help`); Space /
Shift-Space now run `scroll.screen_down|up` (§2.3). Other new commands:
`view.spread_first_alone`, `help.page`; `:layout <name>`, `:help`
(`:h`). New modes: `OUTLINE`, `HELP`; the search prompt shows `/`, the
link question replaces the left of the status bar. Outline and help use
the fixed list keys.

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
- Selection: translucent blue (`#3D7EFF` at 35%). Caret: a red block
  around its character (1.5 px outline, light red fill), like vim's
  Normal/Visual cursor. Changed 2026-10-28 after the user's feedback: the
  first build drew a 2 px bar at the character's left edge, which looked
  like an insert cursor sitting one character behind the selection end.
  Selected annotation: dashed 1 px outline in the annotation's color.
- Overlays (outline, list, help, comment popup): dark panels matching the
  status bar, monospace text, the current row highlighted.
- Fonts: egui's bundled default fonts. No extra font files in v1 (no licence
  bookkeeping needed). If a font is added later, record its licence in
  `ASSETS.md`.
- Dark/recolored page rendering: planned in Phase 3b (below).

### Theming (Phase 3b, as implemented; user's choice "A", 2026-10-28)

Goal: marg's look is fully configurable and shareable, like qutebrowser's
"ricing", without bloat: no new dependencies, no scripting, theme values
resolved once at config load (no per-frame cost). Out of scope (options
B and C, not chosen): runtime `:set`/`:bind`, remappable Insert/Box/list
keys, aliases, userscripts.

**Colors** — `[colors]`, every color marg draws, `#RRGGBB` or
`#RRGGBBAA`. Defaults are today's look:

| Key | Default | Used for |
|---|---|---|
| `background` | `#2B2B2B` | around the pages |
| `page_edge` | `#0000005A` | 1 px line at page edges |
| `statusbar_bg` / `statusbar_fg` | `#1E1E1E` / `#D0D0D0` | status bar, command line |
| `statusbar_warn` / `statusbar_error` | `#FFC060` / `#FF7070` | messages |
| `panel_bg` / `panel_fg` / `panel_dim` | `#1E1E1E` / `#D0D0D0` / `#808080` | lists, editor, tooltips, debug overlay |
| `panel_border` | `#555555` | editor and tooltip border |
| `panel_selected_bg` | `#3A3A3A` | selected list row |
| `editor_selection` | `#2D4E8F` | selected text in the comment editor |
| `hint_bg` / `hint_fg` / `hint_typed` / `hint_border` | `#FFD84D` / `#000000` / `#9A8020` / `#806810` | hint and grid labels |
| `selection` | `#3D7EFF59` | text selection |
| `caret` / `caret_fill` | `#E02020` / `#E0202028` | caret block |
| `search_hit` / `search_current` | `#FF9F1A4D` / `#FF5F1A99` | search hits, the current hit (added in Phase 4) |
| `grid_line` / `grid_cell` | `#3D7EFF5A` / `#3D7EFF3C` | box grid, picked cell |
| `recolor_fg` / `recolor_bg` | `#D0D0D0` / `#1E1E1E` | what black and white become on recolored pages |

The recent list's "missing file" grey becomes `panel_dim`. Category
colors stay in `[[category]]` (they are written into PDFs and must match
Zotero's), not in themes.

**Fonts** — `[fonts]`: `file = ""` (path to a `.ttf`/`.otf` on the
user's system, `~` expanded; empty = egui's built-in monospace), `size =
12.0` (status bar, lists, editor, tooltips), `hint_size = 12.0`. marg
still ships no font files, so no licence bookkeeping. A font that fails to
load falls back to the built-in one with an error message. Startup is
measured with a large font against the 150 ms target.

**Status bar** — `[statusbar]`: `left = ["mode", "file", "page", "zoom",
"layout", "unsaved"]`, `right = ["search", "keys"]` (search hits since
Phase 4; typed hint label or pending key sequence), `separator = "  "`. Messages
still replace the right side for `ui.message_secs`. Unknown parts are
reported at load and skipped. Prompts (`:`, category prompt, Box mode
help) replace the left side as now.

**Theme files** — top-level `theme = "themes/gruvbox.toml"` in
`config.toml` (relative to the config folder, `~` expanded). A theme file
holds only `[colors]` and `[fonts]`; entries in `config.toml` override the
theme's. A broken theme file: error message, defaults for its values.
Two example themes ship in `docs/themes/` (a light one and a dark one),
loaded by a test so they never go stale.

**Dark pages** — `[view] recolor = false`; command `view.recolor`
toggles it for the session (default key `gr`; Zathura's `Ctrl-r` is redo
in marg). Recoloring happens on the document thread: page content is
drawn, tinted (MuPDF `tint`: black → `recolor_fg`, white → `recolor_bg`),
then annotations and form fields are drawn on top in their own colors.
The flag is part of the render key, so toggling re-renders. Risk: MuPDF
draws highlights with multiply blending, which nearly vanishes on a dark
page; check with screenshots in that step and pick a fix (e.g. draw
highlights as overlays from the annotation listing) with the user.

**Steps** (branch `phase-3b-theme`, one commit each): 1 `[colors]` and
the theme struct (all hard-coded colors read from it); 2 theme files; 3
fonts; 4 status bar parts; 5 dark pages; 6 example themes, docs,
handover.

**As implemented (Phase 3b):**

- `[colors]`: `marg-core/src/theme.rs` (`Rgba`, `ColorsConfig`); the app
  converts them once into `marg/src/theme.rs` `Theme` (egui colors and
  the hint font), passed to every draw function. Small changes from
  before: comment text is `panel_fg` (was `#DDDDDD`), the debug overlay
  uses `panel_bg` (was translucent black), the missing-file grey is
  `panel_dim`. Annotation-derived colors (provisional annotations, the
  selected annotation's outline) are not theme colors.
- Theme files: `Config::with_theme` checks the config and the theme file
  separately (errors name the file and line), then fills in the theme's
  values where the config has none (per key). Unknown sections in a
  theme file are an error. A missing or broken theme: the config alone,
  plus an error message. `theme` is resolved against the config folder
  (`~` expanded; absolute paths work).
- `[fonts]`: the file is read and checked with `skrifa` (§9) before egui
  gets it; it becomes the first font of both egui families, egui's own
  fonts fill in missing characters. `size` sets the Monospace, Body and
  Button text styles; `hint_size` the hint and grid labels (both clamped
  to 4–64). The status bar is at least `size + 8` px high. Reading and
  checking: 2.6 ms for a 2.5 MB font, 7.4 ms for a 26 MB CJK collection
  (release); egui builds the font lazily in the first frame, no
  measurable difference to the first page on Xvfb.
- `[statusbar]`: parts `mode`, `file`, `page`, `zoom`, `layout`,
  `unsaved`, `keys` (and `search` since Phase 4, on the right by
  default); empty parts are left out with their separator.
  Unknown names: an error message at start ("statusbar: unknown part …
  (known: …)"), the rest is used. Messages and the comment editor's hint
  still replace the right side.
- Dark pages: `[view] recolor`, `gr` (`view.recolor`, "Dark pages on" /
  "off"). `RenderKey.recolor` (`marg_pdf::Recolor`); `Doc::render` runs
  the page content, tints it (`Pixmap::tint`), runs annotations and
  widgets into a transparent pixmap and composites it (premultiplied
  "over") on top: drawn directly, a 40% yellow highlight came out as
  (30, 28, 18). Cost: 64 → 70 ms for a 876×1263 render. Images are
  recolored like the text (Zathura without "keephue"). Pages still
  rendering are filled with `recolor_bg`; renders in the other coloring
  stand in until the new ones arrive (never blank).
- Example themes: `docs/themes/gruvbox-dark.toml`,
  `docs/themes/solarized-light.toml`; a test loads both and checks that
  they set every `[colors]` key.

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

**As implemented (Phase 1):** sections `[view]`, `[render]`, `[ui]`
(status bar height, message seconds, sequence timeout, recent and state file
limits) and `[keys.normal]`; defaults in `marg-core/src/config.rs`. A broken
config falls back to all defaults and shows the error. Reloading, `:set`,
`:config` and unknown-key reporting come in Phase 5.

**As implemented (Phase 2):** added `[hints]` (`chars`, `mode =
"two-stage" | "words"`, `extra_chars = "qwertyuiop"`), `[annotation]`
(`author`, `highlight_opacity`, `box_stroke_pt`, `autosave_secs`,
`undo_steps = 100`), `[[category]]` (`name`, `color`; a config with any
`[[category]]` replaces all five defaults), `[keys.caret]`,
`[keys.visual]`, and in `[ui]` `multi_click_ms = 400`, `file_check_secs =
1.0`, `drag_scroll_rate = 12.0`, `drag_scroll_edge_px = 16.0`. Backups go to `<data_local_dir>/marg/backups` (Linux
`~/.local/share/marg/backups`, respects `XDG_DATA_HOME`).

**As implemented (Phase 3):** `[annotation]` adds `note_size_pt = 20`,
`box_grid_cols = 9`, `box_grid_rows = 12`, `box_step_pt = 4`,
`box_fine_step_pt = 1`; `[ui]` adds `comment_width_px = 360`,
`comment_max_height = 0.4`, `annot_list_width = 0.35`; new key table
`[keys.annot]`.

**As implemented (Phase 3b):** top-level `theme = ""`; `[view] recolor =
false`; new sections `[colors]`, `[fonts]`, `[statusbar]` (§6
"Theming").

**As implemented (Phase 4):** `[view] jump_history = 100`; `[ui]
outline_width = 0.4`; `[colors] search_hit`, `search_current`;
`[statusbar] right` defaults to `["search", "keys"]`. The example themes
set the new colors.

**As implemented (Phase 5, `:config`):** `:config` (`config.write`)
writes `marg-core/src/default_config.toml` plus the default key tables
(generated from `keymap::DEFAULT_*`, each line with the command's help
text) to the config path, creating the folder; an existing file is
never changed ("… exists; not changed"). Every setting is **commented
out** with its default (`#key = value`, explanations as `# text`), so
untouched settings keep following marg's defaults when they change
(Claude's choice; the alternative is writing them active). Tests: the
file as written parses to the defaults; with every `#key`/`#[table]`
line uncommented it parses to the defaults plus the default keymaps and
has exactly the keys of `Config` (so a new setting must be added to the
file), and every bound command has a help text. Messages show paths
with `~` for the home folder.

**As implemented (Phase 5, errors and reload):** unknown settings
(`config::unknown_settings`: every dotted key not in `Config`; the
`keys.*` tables count as one each) are reported for the config and the
theme file ("config.toml: unknown setting view.zom_step"); the rest is
used. Keymap problems (unknown commands, unparsable keys, conflicts)
were already reported since Phase 1. TOML errors are put on one line
for the status bar ("config.toml: line 4, column 6: unclosed table,
expected `]`"); messages about a file's contents name the file only,
read errors the whole path. **Reload:** every `ui.file_check_secs` the
UI thread compares the modification time and size of the config and
its theme file (two `stat`s); on a change it reads both again. A broken
config keeps the current one ("Config not reloaded: …"); otherwise the
keymaps, status bar parts, colors, fonts, author and dark pages
(`[view] recolor`, `recolor_fg/bg`) change at once ("Config
reloaded", or the first problem). Settings read when a file opens
(`[view]` layout, gap, zoom limits, `spread_first_page_alone`,
`jump_history`, `undo_steps`) apply to the next file; `[render]
cache_mb` on the next start.

**As implemented (Phase 5, `:set`; added 2026-10-29 at the user's
request):** `:set name=value` (`:se`, `config.set`) changes one setting
for the session; `:set name` shows it ("view.zoom_step = 1.1"). Names
are dotted, or the last part alone when unique (`:set zoom_step=1.2`).
The value is TOML, else a string (`words`, `#000000`); the whole config
is checked by deserializing it (`Config::set`/`get` in core), so a bad
value changes nothing and the error names the setting. Not for `keys.*`,
`[[category]]` and `theme` ("change it in the config file"). The app
applies a change like a reload (`apply_config`: keymaps, status bar,
colors, fonts, author, dark pages) and also gives the open view the new
gap and zoom limits (`View::set_spacing`); `:set` values are kept in
`session_sets` and applied again on top of the file after every
reload.

### State file

Keyed by the PDF's permanent document ID (first element of the trailer `/ID`
array), which stays the same across incremental saves. Fallback for PDFs
without an ID: the canonical absolute path. (Not a content hash: that changes
on every save.)

Stored per file: last page and offset, zoom and fit mode, layout, marks, last
opened time, last known path. Recent files list: the 50 most recent.

**As implemented (Phase 1):** keys `id:<hex>` or `path:<canonical path>`;
position is the document point at the window's top-left. Saved when a file
opens, before `:e` switches files, and on exit; each save re-reads the file
and changes only its own entry (several windows can share it). At most 1000
files (`state_files_max`); the least recently opened are dropped.
**As implemented (Phase 5):** also saved every `ui.state_save_secs`
while the entry differs from the last one written (on the UI thread;
the file is small).
**As implemented (Phase 4):** the layout and marks (`[files."…".marks.a]`
= page, x, y) are saved per file; older state files without marks load
(empty).

### Developer tooling

- `:debug` toggles an overlay: frame time (last / p95), render queue length,
  cache usage in MB, last render time per page, open file's MuPDF warnings.
- `MARG_LOG=debug` raises the log level. `MARG_TIMING=1` logs startup phases
  with timestamps (process start → window → document open → first page
  shown), used to check the startup target.
- `:version` shows the version (from `Cargo.toml`) and git commit hash.

**UI checks without a screen:** `scripts/xvfb-drive.sh` runs the release
build on Xvfb, sends keys with `xdotool` and saves screenshots (usage in the
script; Phase 3 added `move`, `click` and `drag` steps). Set
`XDG_STATE_HOME`, `XDG_CONFIG_HOME` (the user's config remaps keys) and
`XDG_DATA_HOME` to scratch dirs. Dead keys cannot be sent there:
`setxkbmap` has no effect on this Xvfb, and xdotool types keysyms missing
from the layout on a spare keycode that winit's keymap (read at startup)
does not know. `cargo run --release -p marg-pdf --example text_timing --
*.pdf` times text extraction of every page.

**As implemented (Phase 1):** `:debug` shows UI frame time (last / p95 over
120 frames), number of renders requested, cache MB and texture count, zoom,
render scale, pixel ratio, tiled or not, and the last render time per
visible page. MuPDF warnings are not captured yet (see §12). `MARG_LOG`
applies to marg's crates; dependencies log warnings only. `MARG_TIMING=1`
logs: window created, document opened, first page shown. The file is opened
on the document thread before the window is created. Measured (release,
Linux/Wayland, this machine): first page at 137–159 ms for papers and a
739-page book; 314 ms for a Springer book whose cover takes ~190 ms to
render.

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
| Font check | `skrifa` (same version and a subset of the features egui uses) | egui panics on a font file it cannot parse; marg checks `[fonts] file` first. Added 2026-10-28 with the user's approval (Phase 3b) |
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
- Release builds: `opt-level = 3`. Local `--release` builds are
  incremental without LTO (changed 2026-10-28 after the user found
  rebuilds slow: ~80 s per change with LTO, ~6 s without, 1 s
  incremental; text extraction and render times measured the same).
  Published binaries use `--profile dist` (`lto = "thin"`,
  `codegen-units = 1`);
  Windows built with the MSVC toolchain and the **dynamic** CRT: `mupdf-sys`
  builds MuPDF's own `platform/win32/mupdf.sln`, whose projects use `/MD`
  (Release) and `/MDd` (Debug), so `+crt-static` would mismatch. The `.exe`
  needs the MSVC runtime: decided by the user 2026-10-29 (Phase 5) to
  ship it that way and say so in the README, instead of patching MuPDF's
  vcxproj files for a static CRT or bundling the runtime DLLs.

**Binary size (Phase 5, measured 2026-10-29, Linux):** release 37.4 MB,
`dist` 30.6 MB (26.1 MB stripped). Most of it is Rust code, led by
`wgpu`/`naga` (eframe's default renderer). A trial with eframe's `glow`
(OpenGL) renderer, without `accesskit` and system fonts, gave 22.9 MB
(18.0 MB stripped), startup the same within noise on Xvfb; it needs only
`on_exit(&mut self, _gl: Option<&glow::Context>)`. **Rejected by the
user** (Claude agreeing): size does not matter for this tool, wgpu's
Vulkan/DX12/Metal fallbacks are safer on untested drivers (and a future
macOS build), and symbol names in backtraces help diagnose crashes, so
`strip` stays off too. For comparison (Arch, installed): Zathura 1.9 MB
plus GTK4 54 MB, GLib 38 MB and a PDF backend (Poppler ~10 MB or libmupdf
56 MB); Okular 19.5 MB plus Qt and KDE Frameworks.

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
  Linux binary and the portable Windows `.exe` to a GitHub release.
  **As implemented (Phase 5):** `.github/workflows/release.yml`: on a
  `v*` tag, Linux and Windows run the tests, build `--profile dist -p
  marg` and upload `marg-linux-x86_64` and `marg-windows-x86_64.exe`; a
  last job creates the release with `gh release create` and generated
  notes. The tag must equal `v` + the workspace version in `Cargo.toml`
  (bump it in a commit before tagging). Windows warns (not fails) if
  `LNK4098` shows up in the release build. "Run workflow" by hand builds
  the artifacts without a release. The Linux binary links the glibc of
  `ubuntu-latest`. Checked locally: `cargo build --profile dist -p
  marg` works (30 MB binary, ~11 min from clean, mostly MuPDF); the
  workflow itself runs only once pushed. Locally
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
| **3. Notes & boxes** | Comment popup (`c`, `e`), notes (`i`), box mode with grid hints, adjust and snap, annotation selection with `ga` (`]a`, `[a`, click and `x` delete were built in Phase 2), edit/recolor/toggle, tooltips, annotation list panel, reading categories from foreign annotations. | The user annotates a full paper (highlights, comments, boxes) and checks them in another viewer and via Zotero's "Import annotations". |
| **3b. Theming** (added 2026-10-28, user's choice "A") | `[colors]` for every drawn color, `[fonts]` (user font file, sizes), configurable status bar parts, `theme = ` files, dark/recolored pages (§6 "Theming"). | The user rices marg with their own theme and reads with dark pages. |
| **4. Reading features** | Single and spread layouts with the defaults rule, outline overlay, search, marks, jump history, link hints and link following, help overlay, `:help` page. | The user reads a slide deck and a book chapter with it. |
| **5. Export & release** | Markdown export, `:config` default file, config auto-reload and error reporting, password prompt, read-only handling, README with the cheat sheet (§2.12), tagged release builds (Linux binary + portable Windows `.exe`), polish from the user's list. | `v1.0.0` is tagged and the user replaces Zathura/Zotero's reader with marg. |

**Phase 4 steps** (branch `phase-4-reading`, planned 2026-10-29 with the
user, one commit each): 1 single and spread layouts (defaults rule,
`Ctrl-l`, `:layout`, per-file layout, spread first page alone; Space
scrolls a screen and turns the page at its edge, like Zathura — user's
choice); 2 jump history and marks; 3 outline overlay; 4 search; 5 links
(`gl`, click, external URLs after `y`, opened with `xdg-open` / `start`);
6 help overlay `?` and `:help`; 7 docs and handover.

**Phase 5 steps** (branch `phase-5-release`, planned 2026-10-29 with the
user, one commit each): 1 Markdown export (`:export md [path]`); 2
`:config` (commented default config); 3 config errors (unknown keys,
keymap conflicts) and auto-reload; 4 password prompt; 5 read-only files;
6 cheat sheet (generated, checked by a test) and README; 7 release
builds in CI on `v*` tags; 8 polish from the user's list (none yet; it
grows as they test), docs and handover. To ask at their step: `:set`
(Phase 1 put it in Phase 5, the Phase 3b choice ruled out runtime
settings), the Windows CRT (§12), the Phase 4 choices.

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
- **`:config` writes every setting commented out** (Phase 5, Claude's
  choice): untouched settings follow later default changes. The other
  way, active values, pins today's defaults.
- Exact grid size and step sizes in box mode.

- **List keys** (§3, "Lists"): built as decided on 2026-10-28; the user
  will re-evaluate after using it. List keys are not remappable yet.
- **Word hint placement** (Phase 2): word labels cover the first letters of
  their word. Alternatives: above the word, or left of it. The user's call
  after trying it.
- **`y` stays in Visual mode** (Phase 2 choice, vim leaves Visual): keeps the
  selection for a highlight after copying. Confirm or change.
- **Highlight keys vs. counts in Visual mode** (raised by the user
  2026-10-28): with `1`–`5` bound, `5l` highlights instead of moving.
  Options: (1) digits are counts, highlights on `g1`–`g5`; (2) digits are
  counts, `Enter` opens a category prompt showing the names, then a digit;
  (3) keep `1`–`5`. Built so all can be combined: option 2 is the command
  `annotate.pick`, bound to `Enter` in Visual by default; options 1 and 3
  are bindings. The user's config frees the digits and binds `g1`–`g5`, so
  they try 1 and 2 side by side (test `visual_digits_can_be_freed_for_counts`).
  Decide the defaults after trying.
- **Words include punctuation** (vim's `W`, not `w`): a selection to a word
  end takes the comma or full stop with it. Confirm or change.
- **Phase 3b choices to confirm**: dark pages recolor images too (a
  "keep hue" option like Zathura's is possible: recolor only, then restore
  image areas from `Request::Images` boxes); `gr` as the key; theme files
  may not set `[statusbar]` (only colors and fonts); the two example
  themes.
- **Phase 5 choices to confirm** (Claude's defaults, the user's call
  after trying): `:export md` paths relative to the working directory
  (like `:e`), a folder gets `<stem>.notes.md`, the date is UTC;
  `:unlock` lasts until the file is closed and is not remembered for
  the next time; read-only shows `[RO]` after the file name (no own
  status bar part); after a config reload, `[view]` settings apply to
  the next opened file and `cache_mb` to the next start; config
  messages name the file only; the cheat sheet is the help page as
  tables (about 200 lines, not one printed page).
- **Phase 4 choices to confirm** (Claude's defaults, the user's call
  after trying): Space in **Continuous** stays next page (Zathura scrolls
  a screen there too; say if it should); the wheel does not turn pages
  in Single/Spread (stops at the page edge, like `j`/`k`); a spread's
  saved position is its left page; outline over the left 40% of the
  view; search colors (orange); Esc in Normal hides the hits; `N` after
  that searches forward from the view; the help filter matches words,
  not fuzzy; `?` closes the help as well as Esc;
  `view.spread_first_alone` has no default key.
- **Phase 3 choices to confirm** (Claude's defaults, the user's call after
  trying): annotation list drawn over the pages instead of beside them;
  note icon above the caret character (or top left of the visible page);
  Esc on Visual `c`'s editor keeps the highlight; `ga` labels on the
  annotation's top-left corner; box grid labels of two or three characters
  (9 hint chars for 108 cells; `extra_chars` would make them shorter);
  grid size and steps (§12 above). Box mode and list keys are fixed, not
  in the keymap.

**Risks**

- ~~**`mupdf` crate API coverage**~~ Checked in Phase 0: no blocking gaps
  (§9 spike report). Two small ones are covered by raw object access.
- **Building MuPDF on Windows** (toolchain, clang/bindgen requirements).
  Checked in Phase 0 with CI on Windows.
- ~~**Hint/caret selection not feeling faster than a mouse.**~~ The core bet.
  The user tried it on real papers and passed the decision milestone on
  2026-10-28 (reading, navigating and selecting "work great").
- **Reading order in multi-column papers** (caret motions, selection across
  columns). Checked in Phase 2 with the two-column fixture and the user's
  papers.
- **Text input in egui** (German umlauts, dead keys, IME on Windows) for the
  comment editor. Phase 3: umlauts and ß typed directly work under Xvfb;
  dead keys could not be simulated there (§8). To check on the user's
  keyboard (Linux) and on Windows.
- **Incremental save edge cases** (repaired files, encrypted files, Windows
  file locks while Zotero has the file open). Encrypted files: checked in
  Phase 5 (tests, AES-256 made by MuPDF; RC4 and real-world files not). Mitigated by backups and the
  rules in §2.9; round-trip tests in Phase 2.
- **Rendering throughput with one document thread.** Measured in Phase 1;
  extra render workers only if needed.

**Known issues**

- Phase 4: every search extracts the text of every page again on the
  document thread (no text cache there); a 700-page book takes about
  as long as extracting its text once (~0.5–1 s at the §2.2 timings,
  streamed, renders first). Not measured on a large real book yet.
- Phase 4: opening external links on Windows (`rundll32
  url.dll,FileProtocolHandler`) is untested; Linux was checked only up to
  the confirmation (no browser on Xvfb).
- Phase 4: link and outline destinations on rotated pages or pages with
  an offset CropBox are untested (boxes are moved by the page bounds'
  origin, like the text).

- Phase 2: lines whose text is invisible (e.g. white header text on a
  colored band) still get line hints. Not filtered on purpose: the text
  layer of OCR'd scans is invisible too, and those lines need hints.
- Phase 2: if another program rewrites the open file in place while marg has
  unsaved changes, MuPDF may read objects it has not loaded yet from the
  changed file; `:w!` in that state could write a broken file (the backup
  stays). `:e!` is the safe choice; replacing a file by rename is safe.
- Phase 2: closing the window when the save fails keeps the window open
  (by design, to not lose changes); `:q!` closes without saving.
- Phase 2: the clipboard after `y` was not checked under Xvfb (no `xclip`);
  copy text is covered by unit tests.

- ~~Phase 3: a comment longer than the tooltip height is cut off~~
  Fixed in Phase 5: the tooltip shows as many rows as fit and ends the
  last in "…" (`e` shows the whole comment).
- Phase 3: Square and Text annotations from other programs can be
  recolored (color and `/Subj`) but not switched with `U` (message).

- MuPDF warnings go to stderr; the `mupdf` crate does not expose
  `fz_set_warning_callback`, so `:debug` cannot list them. Possible via
  `mupdf-sys` FFI in `marg-pdf` (unsafe); not done.
- Heavy first pages (full-page cover images) delay the first render past the
  150 ms target. Re-measured 2026-10-28 (release, Wayland): Springer book
  282–325 ms to first page (cover render ~215 ms at the app's 1.76 scale),
  two magazine-style PDFs 268–291 ms; light pages 132–137 ms. Window ready at
  ~67 ms, document at ~80 ms; the rest is the cover render. A quick
  low-resolution first render could help; not done.
- ~~The state file is written on open, file switch and exit only~~
  Fixed in Phase 5: also every `[ui] state_save_secs` (5) while the
  position, zoom, fit, layout or marks differ from the last write
  (checked on Xvfb: killed after paging, the new page was saved). A
  failing write is reported once, then only logged.
- Tiles re-run the whole page's content per tile (MuPDF clips the drawing
  only). If tiled zoom feels slow, cache a display list per page on the
  document thread.
- Windows AltGr handling and Linux Alt bindings are untested on real
  keyboards (no Alt bindings exist by default).
- Under Xvfb each simulated wheel click arrives twice. Cause (confirmed
  2026-10-28): winit 0.30 X11 turns buttons 4–7 into a wheel event on both
  press and release, ignoring the state. Real mice have XI2 scroll
  valuators, so the X server flags those button events as emulated and winit
  drops them; the XTEST pointer has none, so both get through. Test-only:
  does not affect real mice on X11, and Wayland does not use this code.
  `MARG_LOG=debug` logs each wheel event.
- First CI run on Windows is slow (MuPDF compiled from source, no cache);
  later runs use `rust-cache`.
- Windows **debug** builds warn `LNK4098: defaultlib 'msvcrt' conflicts`:
  `mupdf-sys` links MuPDF's Debug config with the debug CRT (`msvcrtd`)
  while Rust always uses the release CRT. Inherent to `mupdf-sys`; the
  release build is clean (checked in the v1.0.0 release run).

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
- Hint labels on list rows (recent files, outline): one key per visible row
  opens it, like `f` in the page.

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

### 2026-10-29 — Phase 5: export & release
- Done (branch `phase-5-release`, not merged, not pushed): plan agreed
  with the user (§11 "Phase 5 steps"), then one commit per step:
  `:export md`; `:config`; unknown settings and config reload; password
  prompt; read-only files and `:unlock`; README and generated cheat
  sheet; release workflow. No polish list yet (the user collects it
  while testing).
- Decisions: files whose permissions forbid annotations open read-only,
  with `[annotation] ignore_permissions` for all files and a command for
  the open file (user, 2026-10-29; Claude named it `:unlock`). Windows
  `.exe` ships needing the MSVC runtime (user, 2026-10-29). Claude:
  `:config` writes settings commented out (§12).
- Tried and failed: nothing lasting. The `mupdf` crate's
  `permissions()` reports everything allowed for real files (`/P` has
  undefined high bits; `from_bits` fails, falls back to all), so `/P`
  is read directly.
- Checks: unit and round-trip tests (encrypted files made by MuPDF:
  open, incremental and full save keep the encryption; restricted and
  write-protected files); Xvfb with screenshots (export, `:config`,
  reload with good/unknown/broken config, password prompt, wrong
  password, `:e!` without asking again, Esc to the recent list,
  read-only, `:unlock`, save). Local `--profile dist` build. Not
  checked: the release workflow (runs only when pushed), Windows,
  RC4 or real-world encrypted PDFs, a restricted publisher PDF.
- State: builds; 174 tests pass; clippy and fmt clean.
- Added after the handover, at the user's request: `:set` (§8);
  fixed known issues: the reading position is saved every
  `state_save_secs`, long comments end in "…" in tooltips. Binary size
  options measured and rejected (§9 "Binary size").
- Release: merged into `main`, version 1.0.0, pushed. The first CI run
  on `main` failed on Windows only: Git checks Markdown out with CRLF
  there and the cheat sheet test compared bytes; the test now ignores
  `\r` (commit on `main`). Trial release run by hand (green: Windows
  29 min, Linux 12 min), then `v1.0.0` tagged and released with
  `marg-linux-x86_64` (31 MB; links only libc, libm, libgcc) and
  `marg-windows-x86_64.exe` (23 MB). The downloaded Linux binary runs
  (`marg 1.0.0`). The Windows `.exe` was not run.
- Next: the user uses marg daily and collects a polish list.
- Watch out: `MARG_BLESS=1 cargo test -p marg-core cheat` after
  changing default keys or help texts, or the cheat sheet test fails.
  A new config setting must go into `default_config.toml` too (test).
  `Request::Open` now carries the password; the UI keeps the one that
  worked in `OpenDoc.password` (`opening` tracks the pending open).

### 2026-10-29 — Phase 4: reading features
- Done (branch `phase-4-reading`, not merged, not pushed): plan agreed
  with the user (§11 "Phase 4 steps"), then one commit per step: single
  and spread layouts; jump history and marks; outline overlay; search;
  links (`gl`, click, external URLs after `y`); `?` and `:help`; docs.
- Decisions: Space in Single/Spread scrolls a screen and turns the page
  at its edge, like Zathura (user, 2026-10-29). Claude kept Space in
  Continuous as next page (listed in §12).
- Tried and failed: fuzzy (subsequence) filtering in the help page
  matched most long lines; it matches words instead.
- Checks: each step on Xvfb with screenshots (layouts, marks, outline,
  search, `gl`, link click, help). Not checked: speed of search on a
  large book (release, real machine), external links opening a browser,
  Windows.
- State: builds; 158 tests pass; clippy and fmt clean. Tests changed
  with new defaults: the default `[statusbar] right` and the count of
  default `g…` bindings (`gl`).
- Merge: the user asked to merge and push right after the handover
  ("merge and push"), before reporting on the Phase 4 choices; merged
  into `main` and pushed with the branch.
- Next: Phase 5 in a fresh session; ask the user about the §12 Phase 4
  choices when they have used it.
- Watch out: `View` paged layouts keep the shown row in `View::row`;
  `goto_page`/`put_doc_pos_at` switch it, `scroll_by` never does. Search
  hits are kept sorted by page as they stream in (`SearchState`).

### 2026-10-28 — Phase 3b: theming
- Done (branch `phase-3b-theme`, not merged): Phase 3 merged into `main`
  (local) after the user's check. Plan recorded (§6 "Theming", §11), then
  one commit per step: `[colors]`; theme files; `[fonts]`; `[statusbar]`;
  dark pages (`gr`); example themes and docs.
- Decisions: the user approved adding `skrifa` (to check font files,
  since egui panics on a broken one) over a `catch_unwind` workaround.
- Tried and failed: dark pages drawn in one pass made highlights vanish
  (multiply blending on a dark page); fixed with a separate annotation
  layer. A first font timing on Xvfb suggested +70 ms; measured in
  detail it was noise (reading + checking 2.6–7.4 ms).
- State: builds; 138 tests pass; clippy and fmt clean (local). Nothing
  pushed; CI has not run on Phase 3 or 3b.
- User check: the user tried the theming ("tried the theming, merge it
  and push everything"). Merged into `main` and pushed with all branches.
- Build time: the user found `cargo run --release` slow at the last step.
  Local release builds are now incremental without LTO (~80 s → 1–2 s per
  change, same measured speed); LTO moved to `--profile dist` (§9).
- Next: Phase 4 (reading features) in a fresh session.
- Watch out: `PageCache` holds the recolor state (`set_recolor`); the
  paper color under rendering pages follows it.

### 2026-10-28 — Phase 3: notes & boxes
- Done (branch `phase-3-notes`, not merged), one commit per step:
  categories of foreign annotations; `[keys.annot]` recolor and `U`
  toggle, in place (`Edit::Update`); comment editor (`e`, Insert mode);
  Visual `c`; notes `i`; comment tooltips; `ga`; box mode `b` (grid,
  adjust, mouse drag); box snap `s` (`Request::Images`, new fixture
  `figure.pdf`); annotation list `A`. Redo of created annotations now
  restores the same object.
- Decisions: box grid covers the whole view, the box stays on the first
  corner's page (user, asked at the start). Everything else in "Phase 3
  choices to confirm" (§12) is Claude's default.
- Tried and failed: image blocks in the page text (`PRESERVE_IMAGES` on
  every text request): a book cover took 394 ms (was < 80 ms), so images
  became their own request. Dead keys under Xvfb (setxkbmap ignored;
  xmodmap + xdotool use a spare keycode winit does not know).
- Mistakes: `7acdd56` was committed with `cargo fmt --check` failing (the
  check's exit code was lost in a pipe; fixed in `1a9c482`), and
  `7cbfa7a`, `c1d100f`, `e0a17ec` with one failing test (the check
  script used `! cargo test | grep -q FAILED`, which `set -e` ignores;
  the test needed updating for `ga`, fixed in `1fdde07`). Run
  `cargo test` itself and let its exit status stop the commit.
- State: builds; 131 tests pass; clippy and fmt clean (local). Not
  pushed yet, so CI has not run on this branch.
- User check: the user tried the Phase 3 build and said "I am very happy
  with the current state of the project after testing. It feels very
  powerful." Phase 3 merged into `main` locally (not pushed). The "Phase
  3 choices to confirm" in §12 stay open; none was raised.
- Customisation: the user wants marg "customizable and ricable" like
  qutebrowser. Claude proposed A (look: colors, fonts, status bar, theme
  files, dark pages), B (A + every key remappable, `:set`/`:bind`, live
  reload, aliases/chaining) and C (B + userscripts/`:spawn`); the user
  chose **A**, after asking about bloat (answer: no new dependencies,
  binary practically unchanged, a few hundred lines). Recorded as Phase 3b
  (§11, §6 "Theming").
- Watch out: Insert, Box and list keys are fixed (not remappable). The
  annotation list is an overlay. `AnnotSpec` has `stroke_pt` (boxes only).

### 2026-10-28 — Phase 2 (continued): option A, annotation selection
- User chose to stay in Phase 2 until the test week is over (option A),
  and asked to add a help page and a cheat sheet (README) to the concept
  (§2.12; Phases 4 and 5), and reported that the caret looked like an
  insert cursor one character behind the selection.
- Done (branch `phase-2-select`): block caret (§6); `]a` / `[a` / click
  to select annotations, `x` to delete, `u` restores the same object
  (pulled forward from Phase 3); scrolling while dragging a selection to
  the window edge. New request `Annots` (idle scan of all pages, urgent
  for edited pages) and edit `Restore`.
- Tried and failed: commit `1e24770` was made although clippy failed (the
  command chained the commit with `;` after clippy); fixed in `11998a6`.
  Chain checks and commit with `&&`. One Xvfb run hung at graphics setup
  (blank screenshot); the rerun worked. A UI test first used the user's
  real config (digits unbound) and made no highlights.
- Later: the user wanted to clear highlights (the click/`]a` + `x` way was
  not obvious) and chose Visual `d` as an eraser for the selected part
  only ("option B" over removing whole highlights). Built; undo is now one
  type for any mix of deleted and created annotations. `AnnotInfo` got
  opacity and author, `AnnotSpec` the comment. Then, as a second option,
  Visual `D` deletes touched highlights whole (Claude suggested `D` by
  analogy with vim's line-wise Visual `D`).
- State: builds; 117 tests pass; clippy and fmt clean; CI green on Linux
  and Windows for `ea919c7`.
- Decision milestone: the user decided to move on ("I feel like we are
  ready to move on to building the next phase"), a few hours into the test
  week instead of a week; recorded in §0. Phase 2 merged into `main`.
  Still open from §12: Visual highlight keys default (user's config uses
  `g1`–`g5` + `Enter`), `y` stays in Visual, punctuation in words, word
  hint placement, categories.
- Next: Phase 3 in a fresh session (this one is long).

### 2026-10-28 — Phase 2: select & highlight
- Done (branch `phase-2-select`, not merged): Phase 1 merged into `main`
  after CI passed (user said "go ahead on phase 2"). Ten steps, one
  commit each: text model + extraction (`Request::Text`), caret motions and
  selections (`caret.rs`), Caret/Visual keymaps and commands, hints (`f`,
  `F`, two-stage and `words`), Caret and Visual modes, `y`, mouse selection,
  highlights `1`–`5` and underline `U` with provisional overlay and page
  revisions in render keys, saving (backup, incremental/full, autosave,
  `:w`, `:w!`, `q` and window close save first, `:q!`), external-change
  detection and `:e!`, undo/redo. New fixture `tests/fixtures/columns.pdf`
  (two columns, hyphenated break, sentences). `open_timing` also times text
  extraction.
- Decisions: none from the user beyond the go-ahead. Claude's choices
  (open questions in §12): `h`/`l` as character motions; whitespace words;
  `y` stays in Visual; backup at the first edit, and an edit fails if the
  backup fails; window close with a failed save stays open.
- Tried and failed: nothing notable. Clippy `too_many_arguments` on
  `draw_tiles` was fixed by passing a render key instead of an `#[allow]`.
- State: builds; 108 tests pass; clippy and fmt clean on Linux. UI checked
  on Xvfb with fixture copies: hints, caret, visual, `F` across columns,
  sentence motion, mouse drag/double/triple click, highlight + underline
  (also checked in Poppler), autosave, backup, `q` saves first, external
  change reload/conflict, undo/redo. Hints checked on one real paper
  (opened read-only; file unchanged). Feel and speed not judged by Claude.
- Later the same day, at the user's request: `:close` / `:bd` / `Ctrl-w`
  saves, closes the file and shows the recent list (user asked which name;
  Claude suggested `:close` + `:bd` alias + `Ctrl-w`, user left it to
  Claude). The document thread got `Request::Close`.
- User feedback while testing: reading, navigating and selecting work
  well. Counts vs. `1`–`5` in Visual mode: user tries `g1`–`g5` via config
  and asked for option 2 as well; built as `annotate.pick` on `Enter`, so
  both are available at once (§12). Deleting older annotations (`x`, Phase 3) was offered to come
  earlier if needed.
- Next: user uses it on real papers for about a week (decision milestone).
- Watch out: UI test runs must set `XDG_CONFIG_HOME`, `XDG_STATE_HOME`
  and `XDG_DATA_HOME` to scratch dirs (the user's real config changes keys;
  backups go to the data dir), and must only save copies. xdotool sends multi-letter hint labels as separate keys
  (`a s`, not `as`).

### 2026-10-28 — Phase 1 follow-up: list keys, timing, wheel
- Done (branch `phase-1-viewer`): lists open for moving (`j`/`k`, `gg`/`G`,
  `Ctrl-d`/`Ctrl-u`), `/` filters, Enter opens, Esc keeps then clears the
  filter, then quits (`marg-core/src/list.rs`, 8 tests; recent list uses
  it). Re-measured heavy-cover startup; found the Xvfb double-wheel cause.
- Decisions: user chose `j`/`k` to move and `/` to filter in lists (my
  proposal: Enter while filtering opens, Esc keeps the filter); will
  re-evaluate after use. `:` for filtering and telescope-style filter-first
  rejected (reasons in §3 "Lists").
- Findings: the 314 ms heavy-cover startup is real (re-measured 282–325 ms;
  no record of it being a miscalculation). Xvfb double wheel = winit X11
  reports buttons 4–7 on press and release for devices without scroll
  axes (XTEST); real mice and Wayland unaffected (§12).
- State: builds; 75 tests pass; clippy and fmt clean. List checked on Xvfb
  with a demo state dir (8 fake recent entries).
- Next: user checks Phase 1; merge; then Phase 2 after the go-ahead.
- Watch out: in a non-interactive `sh`, a background job's stdin is
  `/dev/null`, so feed `xvfb-drive.sh` its steps directly, not from `&`.

### 2026-10-27 — Phase 1: viewer
- Done (branch `phase-1-viewer`, not merged): config defaults, paths,
  logging; document thread with page/tile renders and replace-queue
  semantics; continuous layout/zoom/fit model; viewer with texture LRU,
  pre-render ±2, nearest-scale fallback, tiles above 200%; command registry,
  keymap with remaps, counts, sequences, timeout; status bar; `:` command
  line with history and `:e` Tab completion; state file (position, zoom,
  fit per `/ID`); recent files list with fuzzy filter; `:debug` overlay;
  `MARG_TIMING`. New fixture `tests/fixtures/boxes.pdf` (Rotate, CropBox,
  offset/inherited MediaBox, UserUnit). Steps 5 and 6 (keymap, command
  line) were one commit.
- Decisions: none from the user this session ("continue" taken as the
  go-ahead for Phase 1). Claude's choices: page sizes read from page
  dictionaries (fast open); RGBA renders on the doc thread; arrow/Page/Home/End
  keys added to the defaults; in lists typing filters and Ctrl-n/p move
  (open question in §12).
- Tried and failed: opening by loading every page took 366 ms for a
  1316-page book; replaced by reading page dictionaries (18 ms; identical
  sizes on 51 real PDFs, opened read-only from `~/Downloads`).
- State: builds; 65 tests pass; clippy and fmt clean on Linux. Release
  smoke runs on fixtures and real PDFs (read-only) work; startup 137–159 ms
  to first page. Visuals not checked by Claude (no screenshot tool on
  COSMIC/Wayland). Windows CI runs after push.
- UI test via Xvfb + xdotool (user installed xdotool) found and fixed:
  Space inserted twice in the command line and recent filter; fit page not
  showing the current page whole; recent filter ranking path matches above
  file-name matches (now name first, `fuzzy::filter_paths`).
- Next: user checks Phase 1; merge; then Phase 2.
- Watch out: eframe 0.37 has no `glow` here, so `App::on_exit(&mut self)`;
  `egui::Id::new` is deprecated (use `Id::unique`); egui-winit drops text
  events while Ctrl is held. Test runs of the app should set
  `XDG_STATE_HOME` to a scratch dir so the user's recent list stays clean.

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

# marg cheat sheet

<!-- cheat sheet: generated, MARG_BLESS=1 cargo test updates it -->

The default keys. `?` shows the keys of the current mode and `:help` everything, with your own bindings.

## Normal — moving

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

## Normal — selecting

| Keys | |
|---|---|
| `f` | hint: place caret |
| `F` | hint: select range |
| `v` | hint: select word |
| `V` | hint: select line |
| `Ctrl-q` | hint: select block |

## Normal — annotating

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

## Normal — files

| Keys | |
|---|---|
| `:` | command line |
| `q` | quit |
| `Ctrl-w` | close the file, show recent files |
| `?` | keys of this mode |

## Caret — selecting

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
| `Ctrl-q` | select a block |

## Visual — selecting

| Keys | |
|---|---|
| `o` | swap selection end |
| `y` | copy text |
| `gs` | search the web for the text |

## Visual — annotating

| Keys | |
|---|---|
| `1-5` | highlight, category 1-5 |
| `U` | underline |
| `Enter` | highlight: choose category |
| `d` | remove highlights from the selection |
| `D` | delete highlights the selection touches |
| `c` | highlight + comment |

## Selected annotation — annotating

| Keys | |
|---|---|
| `1-5` | recolor: category 1-5 |
| `e` | edit comment |
| `U` | switch highlight / underline |

## Hints

| Keys | |
|---|---|
| `label` | pick that target |
| `Backspace` | remove the last typed character |
| `Esc` | cancel |

Text hints come in two stages: every visible line gets a label at its left, then each word of the picked line gets one. f puts the caret on the word, F selects from one word to another, v selects the word, V a whole line (line labels only), and Ctrl-q starts a block selection (the text in a rectangle) on the word.

## Box mode

| Keys | |
|---|---|
| `label label` | the cells of two corners |
| `h j k l` | move the bottom-right corner (Ctrl: finely) |
| `H J K L` | move the top-left corner (Ctrl-Shift: finely) |
| `s` | snap to the image under the box |
| `1-5` | category |
| `Enter / Esc` | save / cancel |

## Comment editor

| Keys | |
|---|---|
| `Enter` | new line |
| `Ctrl-Enter` | save |
| `Esc` | cancel (twice when the text changed) |

## Lists (recent files, outline, annotations, help)

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

## Search prompt

| Keys | |
|---|---|
| `text` | search as you type (case matters only with capitals) |
| `Enter` | keep the search; n / N go to the next / previous hit |
| `Enter on an empty prompt` | search for the last query again |
| `Esc` | back to where the search started |
| `Esc in Normal mode` | hide the hits (n searches again) |

## Commands (:)

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

## Categories

| Keys | |
|---|---|
| `1` | Key point |
| `2` | Question |
| `3` | Definition |
| `4` | Disagree |
| `5` | Evidence |

The category name is written into the annotation's subject; other programs' annotations get a category by subject, else by the nearest color.
<!-- end of cheat sheet -->

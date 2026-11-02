#!/bin/sh
# Build marg with the dist profile and install it for the current user
# (Linux): the binary in ~/.local/bin, the desktop entry and the icon in
# ~/.local/share. Safe while marg runs: open windows keep the old binary
# until they are restarted.
#
#   scripts/install.sh                 build and install
#   scripts/install.sh --default-app   also make marg the default PDF app
#                                      (mimeapps.list, and COSMIC's
#                                      cosmic-mimeapps.list if it exists)
set -eu

default_app=false
for arg in "$@"; do
    case $arg in
        --default-app) default_app=true ;;
        -h | --help) sed -n '2,10s/^# \{0,1\}//p' "$0"; exit 0 ;;
        *) echo "unknown option: $arg" >&2; exit 2 ;;
    esac
done

root=$(cd "$(dirname "$0")/.." && pwd)
assets=$root/crates/marg/assets
bin=$HOME/.local/bin
data=${XDG_DATA_HOME:-$HOME/.local/share}
config=${XDG_CONFIG_HOME:-$HOME/.config}

cargo build --manifest-path "$root/Cargo.toml" --profile dist -p marg

# Copy, then rename: a running marg keeps its file, nothing is half written.
install_file() { # <mode> <source> <target>
    mkdir -p "$(dirname "$3")"
    cp "$2" "$3.new"
    chmod "$1" "$3.new"
    mv "$3.new" "$3"
}
install_file 755 "$root/target/dist/marg" "$bin/marg"
install_file 644 "$assets/marg.desktop" "$data/applications/marg.desktop"
install_file 644 "$assets/marg.svg" "$data/icons/hicolor/scalable/apps/marg.svg"
install_file 644 "$assets/marg-256.png" "$data/icons/hicolor/256x256/apps/marg.png"

# Optional: desktops without the MIME cache read the entries directly.
update-desktop-database "$data/applications" 2>/dev/null || true
# Refresh an icon cache that is there; do not create one (a cache that
# other programs do not update would hide their icons).
if [ -f "$data/icons/hicolor/icon-theme.cache" ]; then
    gtk-update-icon-cache -q "$data/icons/hicolor" 2>/dev/null || true
fi

# Sets application/pdf=marg.desktop under [Default Applications] and
# leaves every other line alone.
set_default() { # <mimeapps file>
    [ -f "$1" ] || printf '[Default Applications]\n' > "$1"
    awk '
        /^\[/ {
            if (in_default && !done) { print "application/pdf=marg.desktop"; done = 1 }
            in_default = ($0 == "[Default Applications]")
        }
        in_default && /^application\/pdf=/ {
            if (!done) print "application/pdf=marg.desktop"
            done = 1
            next
        }
        { print }
        END {
            if (!done) {
                if (!in_default) print "[Default Applications]"
                print "application/pdf=marg.desktop"
            }
        }
    ' "$1" > "$1.new"
    mv "$1.new" "$1"
    echo "default PDF app: marg ($1)"
}
if $default_app; then
    mkdir -p "$config"
    set_default "$config/mimeapps.list"
    [ -f "$config/cosmic-mimeapps.list" ] && set_default "$config/cosmic-mimeapps.list"
fi

echo "installed: $("$bin/marg" --version)"
case :$PATH: in
    *:"$bin":*) ;;
    *) echo "note: $bin is not on PATH" >&2 ;;
esac

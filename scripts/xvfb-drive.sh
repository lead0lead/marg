#!/bin/sh
# Drive a release build of marg on a virtual X display and take screenshots.
# Needs Xvfb, xdotool and ImageMagick. Run it through xvfb-run, e.g.:
#
#   env -u WAYLAND_DISPLAY XDG_STATE_HOME=/tmp/marg-ui xvfb-run -a \
#     -s "-screen 0 1280x900x24" scripts/xvfb-drive.sh out/ file.pdf <<'STEPS'
#   keys 3 J colon d e b u g Return
#   shot debug
#   STEPS
#
# Arguments: <outdir> [marg args...]. Steps on stdin, one per line:
#   keys <xdotool key names...>   type <text>   shot <name>
#   wheel <button 4|5> [ctrl]     (arrives twice: winit X11 counts press and
#                                  release when the device has no scroll axes)
# Rendering is software (llvmpipe): judge looks here, never speed.
out=$1; shift
mkdir -p "$out"
"$(dirname "$0")/../target/release/marg" "$@" > "$out/log.txt" 2>&1 &
pid=$!
sleep 3
win=$(xdotool search --name marg | head -1)
xdotool windowactivate --sync "$win" 2>/dev/null; xdotool windowfocus "$win"
while read -r cmd rest; do
  case "$cmd" in
    keys) xdotool key --delay 80 $rest ;;
    type) xdotool type --delay 80 "$rest" ;;
    wheel) set -- $rest; xdotool mousemove 300 300; if [ "$2" = ctrl ]; then xdotool keydown ctrl click $1 keyup ctrl; else xdotool click $1; fi ;;
    shot) sleep 1; import -window root "$out/$rest.png" ;;
  esac
done
kill $pid 2>/dev/null; wait $pid 2>/dev/null

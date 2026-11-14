#!/usr/bin/env bash
# Record the README demo headlessly through Rune's tooling.
#
# Everything is synthetic and private: curated .desktop entries, a curated
# PATH, and a private cliphist database seeded with demo strings. The recording
# never reads the live clipboard, the real app list or the owner's state.
#
# Usage:
#   scripts/record-demo.sh --binary target/release/gofi [--out docs/assets]
#
# Outputs: <out>/demo.mp4, <out>/launcher.png
# Work dir: target/evidence/demo (fixtures, frames, logs).

set -euo pipefail

BINARY="target/release/gofi"
OUT="docs/assets"
FPS=60

while [ $# -gt 0 ]; do
  case "$1" in
    --binary) BINARY="${2:-}"; shift 2 ;;
    --out) OUT="${2:-}"; shift 2 ;;
    -h|--help)
      echo "usage: scripts/record-demo.sh [--binary BIN] [--out DIR]"
      exit 0
      ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[ -x "$BINARY" ] || { echo "not executable: $BINARY" >&2; exit 2; }
BINARY=$(realpath "$BINARY")
for tool in ffmpeg cliphist identify; do
  command -v "$tool" >/dev/null || { echo "missing: $tool" >&2; exit 2; }
done
if command -v magick >/dev/null; then
  MAGICK=magick
elif command -v convert >/dev/null; then
  MAGICK=convert
else
  echo "missing: ImageMagick (magick or convert)" >&2
  exit 2
fi

REPO=$(cd "$(dirname "$0")/.." && pwd)
WORK="$REPO/target/evidence/demo"
rm -rf "$WORK"
mkdir -p "$WORK/data/applications" "$WORK/bin" "$WORK/state" "$WORK/runtime" \
  "$WORK/home" "$WORK/config" "$OUT"
chmod 700 "$WORK/runtime"

# The app list is fixture data, not the host's. Icons and the Vulkan ICD come
# from /usr/share, so the headless renderer still finds both.
[ -d /usr/share/icons ] || { echo "missing: /usr/share/icons" >&2; exit 2; }
[ -d /usr/share/vulkan ] || { echo "missing: /usr/share/vulkan" >&2; exit 2; }
ln -s /usr/share/icons "$WORK/data/icons"
ln -s /usr/share/vulkan "$WORK/data/vulkan"

while IFS=$'\t' read -r name exec icon; do
  [ -n "$name" ] || continue
  slug=$(printf '%s' "$name" | tr '[:upper:] ' '[:lower:]-')
  cat > "$WORK/data/applications/demo-$slug.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=$name
Exec=$exec
Icon=$icon
Categories=Utility;
EOF
done <<'APPS'
Blender	blender	blender
btop	btop	btop
Discord	discord	discord
Firefox	firefox %u	firefox
GIMP	gimp	gimp
Ghostty	ghostty	com.mitchellh.ghostty
htop	htop	htop
Kitty	kitty	kitty
Krita	krita	krita
LibreOffice Calc	libreoffice --calc	libreoffice-calc
LibreOffice Writer	libreoffice --writer	libreoffice-writer
OBS Studio	obs	com.obsproject.Studio
APPS

# Run mode gets a short PATH of real commands, so it does not list the host's
# executables. cliphist stays for clipboard mode; it is part of gofi's design.
for cmd in cargo git ffmpeg ffplay ffprobe grim just jq rg sway wtype; do
  path=$(command -v "$cmd" 2>/dev/null) || continue
  ln -sf "$path" "$WORK/bin/$cmd"
done
CLIPHIST=$(command -v cliphist)
cat > "$WORK/bin/cliphist" <<EOF
#!/bin/sh
exec "$CLIPHIST" -db-path "$WORK/cliphist.db" "\$@"
EOF
chmod +x "$WORK/bin/cliphist"

# Synthetic clipboard entries: safe to publish, and never the live history.
while IFS= read -r entry; do
  printf '%s' "$entry" | cliphist -db-path "$WORK/cliphist.db" store
done <<'CLIPS'
docs/decisions/0010-quiet-motion.md
Cargo.lock
MIT License
Lorem ipsum dolor sit amet
~/code/my/gofi
cliphist list | wl-copy
A quick brown fox jumps over the lazy dog.
swaymsg reload
ffmpeg -i clip.mp4 -vf fps=30 demo.gif
cargo test -p gofi
https://github.com/geril07/gofi
git rebase --interactive
CLIPS

# Searches, scrolls and tab switches. `still` is the README screenshot;
# `end` closes the loop on the same resting state it started in.
cat > "$WORK/scenario.json" <<'SCENARIO'
[
  {"t": 0.3, "type": "mark", "name": "start"},
  {"t": 0.8, "type": "type", "text": "firef", "every": 0.12},
  {"t": 1.5, "type": "mark", "name": "still"},
  {"t": 1.9, "type": "key", "name": "w", "cmd": true},
  {"t": 2.3, "type": "key", "name": "Down"},
  {"t": 2.6, "type": "key", "name": "Down"},
  {"t": 2.9, "type": "key", "name": "PageDown"},
  {"t": 3.3, "type": "key", "name": "Home"},
  {"t": 3.8, "type": "key", "name": "Tab"},
  {"t": 4.2, "type": "type", "text": "ff", "every": 0.16},
  {"t": 5.0, "type": "key", "name": "Down"},
  {"t": 5.6, "type": "key", "name": "Tab"},
  {"t": 6.1, "type": "key", "name": "Down"},
  {"t": 6.4, "type": "key", "name": "Down"},
  {"t": 6.7, "type": "key", "name": "PageDown"},
  {"t": 7.4, "type": "key", "name": "Tab", "shift": true},
  {"t": 7.7, "type": "key", "name": "Tab", "shift": true},
  {"t": 8.1, "type": "type", "text": "gh", "every": 0.15},
  {"t": 8.9, "type": "key", "name": "w", "cmd": true},
  {"t": 9.4, "type": "mark", "name": "end"}
]
SCENARIO

"$MAGICK" -size 1920x1200 radial-gradient:'#2b2b3d-#0b0b10' -depth 8 "$WORK/backdrop.png"

demo_env() {
  env -u WAYLAND_DISPLAY -u DISPLAY -u SWAYSOCK -u I3SOCK \
    HOME="$WORK/home" \
    XDG_DATA_HOME="$WORK/data" XDG_DATA_DIRS="$WORK/data" \
    XDG_STATE_HOME="$WORK/state" XDG_CONFIG_HOME="$WORK/config" \
    XDG_RUNTIME_DIR="$WORK/runtime" \
    PATH="$WORK/bin" \
    "$@"
}

# The video and the still are separate runs so the still keeps its transparent
# corners; both replay the same scenario.
demo_env "$BINARY" --record \
  --desktop "$WORK/backdrop.png" \
  --script "$WORK/scenario.json" \
  --video "$WORK/demo.mp4" \
  --duration auto \
  --fps "$FPS" \
  --scale 2 >"$WORK/render.log" 2>&1

demo_env "$BINARY" --record \
  --script "$WORK/scenario.json" \
  --snap "$WORK/launcher.png" \
  --at mark:still \
  --scale 2 >>"$WORK/render.log" 2>&1

cp "$WORK/demo.mp4" "$OUT/demo.mp4"
cp "$WORK/launcher.png" "$OUT/launcher.png"

# The README player uses a smaller, broadly decodable copy: 1280x800 at 30 fps,
# H.264 Main@4.0. The 1920x1200 60 fps file stays as the master.
ffmpeg -v error -y -i "$OUT/demo.mp4" \
  -vf "scale=1280:800:flags=lanczos,fps=30" \
  -c:v libx264 -profile:v main -level:v 4.0 -pix_fmt yuv420p \
  -crf 20 -preset slow -movflags +faststart -an "$OUT/demo-web.mp4"

[ "$(identify -format '%wx%h' "$OUT/launcher.png" | head -1)" = "1600x992" ] \
  || { echo "launcher.png is not 1600x992" >&2; exit 1; }
[ "$(ffprobe -v error -select_streams v:0 -show_entries stream=width,height,r_frame_rate -of csv=p=0 "$OUT/demo.mp4")" = "1920,1200,60/1" ] \
  || { echo "demo.mp4 is not 1920x1200 at 60 fps" >&2; exit 1; }
[ "$(ffprobe -v error -select_streams v:0 -show_entries stream=profile,width,height,r_frame_rate -of csv=p=0 "$OUT/demo-web.mp4")" = "Main,1280,800,30/1" ] \
  || { echo "demo-web.mp4 is not H.264 Main 1280x800 at 30 fps" >&2; exit 1; }

printf 'demo.mp4      %sx%s  %s fps  %s KB\n' 1920 1200 "$FPS" "$(( $(stat -c %s "$OUT/demo.mp4") / 1024 ))"
printf 'demo-web.mp4  1280x800  30 fps  %s KB\n' "$(( $(stat -c %s "$OUT/demo-web.mp4") / 1024 ))"
printf 'launcher      1600x992\n'
printf 'work          %s\n' "$WORK"

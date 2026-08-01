#!/usr/bin/env bash
# Assemble the 3-minute demo video: slides + narration segments -> MP4
set -euo pipefail
cd "$(dirname "$0")"

FFMPEG=ffmpeg
FFPROBE=ffprobe
SLIDES=slides
AUDIO=audio
SEG=segments
OUT=neuron-wire-demo-v0.3.1.mp4
mkdir -p "$SEG"

PAD=0.7   # seconds of silence padding per segment

# 1) per-segment durations from audio
declare -a DURS
i=1
for f in "$AUDIO"/s0*.mp3; do
  dur=$("$FFPROBE" -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$f")
  DURS[$i]=$dur
  i=$((i+1))
done

# 2) build each segment: loop slide for (audio + pad)
i=1
for f in "$AUDIO"/s0*.mp3; do
  slide=$(printf "s%02d.png" "$i")
  dur=$(python -c "print(${DURS[$i]} + $PAD)")
  "$FFMPEG" -y -v error -loop 1 -framerate 30 -t "$dur" -i "$SLIDES/$slide" -i "$f" \
    -c:v libx264 -preset medium -tune stillimage -pix_fmt yuv420p -r 30 \
    -c:a aac -b:a 160k -shortest \
    -vf "scale=1280:720:force_original_aspect_ratio=decrease,pad=1280:720:(ow-iw)/2:(oh-ih)/2" \
    "$SEG/seg$i.mp4"
  echo "seg$i.mp4 dur=$dur"
  i=$((i+1))
done

# 3) concat segments (native Windows ffmpeg needs Windows-style paths)
: > "$SEG/list.txt"
for f in "$SEG"/seg*.mp4; do
  win=$(cygpath -m "$PWD/$f")
  echo "file '$win'" >> "$SEG/list.txt"
done
"$FFMPEG" -y -v error -f concat -safe 0 -i "$SEG/list.txt" -c copy "$OUT"

echo "DONE: $OUT"
"$FFPROBE" -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$OUT"

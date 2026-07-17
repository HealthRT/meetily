#!/usr/bin/env bash
# Transcribe audio locally with mlx-whisper (Apple Silicon Metal), then optionally summarize.
# Usage:
#   ./scripts/transcribe.sh recording.m4a
#   ./scripts/transcribe.sh recording.wav --summarize
set -euo pipefail
export PATH="/usr/local/bin:$HOME/Library/Python/3.9/bin:$PATH"

AUDIO="${1:-}"
DO_SUMMARIZE=0
[[ "${2:-}" == "--summarize" ]] && DO_SUMMARIZE=1

if [[ -z "$AUDIO" || ! -f "$AUDIO" ]]; then
  echo "Usage: $0 <audio-file> [--summarize]"
  exit 1
fi

if ! command -v mlx_whisper >/dev/null 2>&1 && ! python3 -c "import mlx_whisper" 2>/dev/null; then
  echo "Installing mlx-whisper (Apple Silicon Whisper)..."
  python3 -m pip install --user -U mlx-whisper
fi

OUT_DIR="$(cd "$(dirname "$AUDIO")" && pwd)"
BASE="$(basename "$AUDIO")"
BASE="${BASE%.*}"
TXT="$OUT_DIR/${BASE}.transcript.txt"

echo "Transcribing with mlx-whisper large-v3-turbo (local, Metal)..."
python3 - <<PY
from pathlib import Path
import mlx_whisper

audio = Path(r"""$AUDIO""")
out = Path(r"""$TXT""")
result = mlx_whisper.transcribe(
    str(audio),
    path_or_hf_repo="mlx-community/whisper-large-v3-turbo",
)
text = (result.get("text") or "").strip()
out.write_text(text + "\n", encoding="utf-8")
print(f"Wrote {out} ({len(text)} chars)")
PY

if [[ "$DO_SUMMARIZE" -eq 1 ]]; then
  ROOT="$(cd "$(dirname "$0")/.." && pwd)"
  "$ROOT/scripts/summarize.sh" "$TXT" clinical-summary
fi

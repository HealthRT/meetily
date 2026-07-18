#!/usr/bin/env bash
# Summarize a meeting transcript with the clinical Ollama model.
# Usage:
#   ./scripts/summarize.sh path/to/transcript.txt
#   ./scripts/summarize.sh path/to/transcript.txt clinical-summary-fast
set -euo pipefail
export PATH="/usr/local/bin:$PATH"

TRANSCRIPT="${1:-}"
MODEL="${2:-clinical-summary}"

if [[ -z "$TRANSCRIPT" || ! -f "$TRANSCRIPT" ]]; then
  echo "Usage: $0 <transcript.txt> [model]"
  echo "Models: clinical-summary | clinical-summary-fast | meeting-summary | medgemma:27b"
  exit 1
fi

if ! curl -sf http://127.0.0.1:11434/api/version >/dev/null; then
  open -a Ollama 2>/dev/null || true
  sleep 2
fi

PROMPT="Summarize the following clinical meeting transcript for charting support.

---
$(cat "$TRANSCRIPT")
---"

ollama run "$MODEL" "$PROMPT"

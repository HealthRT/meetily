#!/usr/bin/env bash
set -euo pipefail
export PATH="/usr/local/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

echo "Waiting for Ollama..."
for i in $(seq 1 30); do
  curl -sf http://127.0.0.1:11434/api/version >/dev/null && break
  open -a Ollama 2>/dev/null || true
  sleep 1
done
curl -sf http://127.0.0.1:11434/api/version >/dev/null || {
  echo "Ollama is not running. Open /Applications/Ollama.app and retry."
  exit 1
}

echo "Pulling models (this can take a while)..."
ollama pull medgemma:4b
ollama pull qwen2.5:14b
ollama pull medgemma:27b

echo "Creating clinical Modelfile aliases..."
ollama create clinical-summary -f "$ROOT/Modelfile.clinical-summary"
ollama create clinical-summary-fast -f "$ROOT/Modelfile.clinical-summary-fast"
ollama create meeting-summary -f "$ROOT/Modelfile.meeting-summary"

echo
ollama list
echo
echo "Ready. Examples:"
echo "  ollama run clinical-summary"
echo "  ollama run clinical-summary-fast"
echo "  ollama run meeting-summary"

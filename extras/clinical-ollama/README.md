# Clinical Ollama (M1 Max 32GB)

Local stack for **clinical meeting summaries** on Apple Silicon. Transcription is Whisper (MLX); summarization is Ollama + MedGemma.

## Hardware fit

| Component | Choice | Why |
|-----------|--------|-----|
| Chip / RAM | M1 Max · 32 GB unified | Comfortably holds ~17–20 GB models + long context |
| Primary LLM | `medgemma:27b` (~17 GB) | Google MedGemma — clinical text + medical imagery |
| Fast LLM | `medgemma:4b` (~3.3 GB) | Quick drafts / short huddles |
| General backup | `qwen2.5:14b` (~9 GB) | Strong long-form meeting structure |
| STT | `mlx-whisper` large-v3-turbo | Local Metal ASR — Ollama does not transcribe audio |

## Install status

- Ollama **0.31.2** → `/Applications/Ollama.app` + CLI `/usr/local/bin/ollama`
- API: `http://127.0.0.1:11434`

## One-time setup

```bash
chmod +x scripts/*.sh
./scripts/setup-models.sh   # pulls models + creates Modelfile aliases
python3 -m pip install --user -U mlx-whisper
export PATH="$HOME/Library/Python/3.9/bin:$PATH"   # mlx_whisper CLI
```

Creates aliases:

- `clinical-summary` → MedGemma 27B, 32K context, clinical system prompt
- `clinical-summary-fast` → MedGemma 4B, 16K context
- `meeting-summary` → Qwen2.5 14B general meeting notes

## Daily use

```bash
# Summarize an existing transcript
./scripts/summarize.sh path/to/transcript.txt

# Transcribe audio locally
./scripts/transcribe.sh recording.m4a

# Transcribe + clinical summary
./scripts/transcribe.sh recording.m4a --summarize

# Interactive chat
ollama run clinical-summary
```

## Important limits

- **Not a medical device.** Outputs need clinician review before charting.
- Prefer **local-only** for PHI; do not send transcripts to cloud APIs.
- Raise `num_ctx` in the Modelfiles if hour-long transcripts truncate (RAM tradeoff).
- Older medical models (`meditron`, aggressive Q2 MedX quants) are weaker for long-form summarization than MedGemma 27B on this machine.

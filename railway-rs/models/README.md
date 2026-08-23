# Local AI model files

The in-process micro-LLM backend (`RAILWAY_AI_BACKEND=local` / `local-first`)
loads two files from this directory:

- `trainbro.gguf` — quantized GGUF weights (llama or qwen2 architecture;
  e.g. HuggingFaceTB/SmolLM2-360M-Instruct-GGUF `Q4_K_M`, ~230 MB)
- `tokenizer.json` — the matching HF tokenizer from the model repo

They are intentionally NOT committed to git. Download them once:

    curl -L -o models/trainbro.gguf \
      https://huggingface.co/bartowski/SmolLM2-360M-Instruct-GGUF/resolve/main/SmolLM2-360M-Instruct-Q4_K_M.gguf
    curl -L -o models/tokenizer.json \
      https://huggingface.co/HuggingFaceTB/SmolLM2-360M-Instruct/resolve/main/tokenizer.json

(The official HuggingFaceTB GGUF repo only ships Q8_0 (~380 MB); the
bartowski mirror carries Q4_K_M (~260 MB) which fits tighter hosts.)

Point `RAILWAY_LOCAL_MODEL_PATH` at any other GGUF to swap models.

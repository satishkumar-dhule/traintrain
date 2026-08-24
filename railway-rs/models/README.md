# Local AI model files

The in-process micro-LLM backend (`RAILWAY_AI_BACKEND=local` / `local-first`)
loads two files from this directory:

- `trainbro.gguf` — quantized GGUF weights (llama or qwen2 architecture;
  e.g. HuggingFaceTB/SmolLM2-135M-Instruct-GGUF `Q4_K_M`, ~105 MB)
- `tokenizer.json` — the matching HF tokenizer (shared across SmolLM2 sizes)

They are intentionally NOT committed to git. Download them once:

    curl -L -o models/trainbro.gguf \
      https://huggingface.co/bartowski/SmolLM2-135M-Instruct-GGUF/resolve/main/SmolLM2-135M-Instruct-Q4_K_M.gguf
    curl -L -o models/tokenizer.json \
      https://huggingface.co/HuggingFaceTB/SmolLM2-135M-Instruct/resolve/main/tokenizer.json

(The official HuggingFaceTB GGUF repo only ships Q8_0; the bartowski mirror
carries Q4_K_M which fits tighter hosts. Larger models like the 360M variant
work with the same loader — just point `RAILWAY_LOCAL_MODEL_PATH` at them.)

## Measured performance (4-core x86_64 sandbox, AVX2, target-cpu=native)

135M Q4_K_M via candle: load ~1.5 s, prefill ~25 ms/token, decode ~7-12 tok/s
(decode is memory-bandwidth-bound and barely scales with threads). A full
tool-calling chat turn lands around 30-90 s on this class of host; the zen
upstream fallback covers latency-sensitive paths.

Note: candle's quantized kernels are scalar-ish ggml ports. Building with
`-C target-cpu=native` (see `.cargo/config.toml`) is REQUIRED — without SIMD
codegen prefill is ~40x slower. Swapping the engine to llama.cpp bindings is
the known path to a further ~5-10x speedup.

After toolchain or model changes, re-check raw engine speed with:

    cargo run --release --bin llm_bench -- models/trainbro.gguf

It prints load time, prefill ms/token and decode tok/s (and flags whether the
binary was built with AVX/NEON codegen).

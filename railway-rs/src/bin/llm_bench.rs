//! Standalone speed probe for the local GGUF engine stack (candle +
//! quantized_llama). Use it to verify SIMD codegen and decode/prefill rates
//! after toolchain or model changes; see models/README.md for reference
//! numbers. Run: cargo run --release --bin llm_bench -- [gguf_path]

use std::error::Error;
use std::time::Instant;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::models::quantized_llama::ModelWeights;
use tokenizers::Tokenizer;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "models/trainbro.gguf".into());
    println!(
        "avx={} neon={} simd128={}",
        candle_core::utils::with_avx(),
        candle_core::utils::with_neon(),
        candle_core::utils::with_simd128()
    );
    let t0 = Instant::now();
    let mut file = std::fs::File::open(&path)?;
    let content = gguf_file::Content::read(&mut file)?;
    let device = Device::Cpu;
    let mut model = ModelWeights::from_gguf(content, &mut file, &device)?;
    println!("load: {:?}", t0.elapsed());
    let tok = Tokenizer::from_file("models/tokenizer.json").map_err(|e| e.to_string())?;
    let prompt = "<|im_start|>system\nYou are Train Bro.<|im_end|>\n<|im_start|>user\nWhich trains run from New Delhi NDLS to Kanpur CNB tonight?<|im_end|>\n<|im_start|>assistant\n";
    let ids = tok
        .encode(prompt, true)
        .map_err(|e| e.to_string())?
        .get_ids()
        .to_vec();
    println!("prompt tokens: {}", ids.len());

    // Prefill
    let input = Tensor::new(ids.as_slice(), &device)?.unsqueeze(0)?;
    let t1 = Instant::now();
    let logits = model.forward(&input, 0)?;
    println!(
        "prefill {}: {:?} ({:.2} ms/tok)",
        ids.len(),
        t1.elapsed(),
        t1.elapsed().as_millis() as f64 / ids.len() as f64
    );

    // Decode steps
    let mut next = logits
        .squeeze(0)?
        .argmax(candle_core::D::Minus1)?
        .to_scalar::<u32>()?;
    let mut pos = ids.len();
    let t2 = Instant::now();
    let steps = 16;
    for _ in 0..steps {
        let inp = Tensor::new(&[next], &device)?.unsqueeze(0)?;
        let lg = model.forward(&inp, pos)?;
        next = lg
            .squeeze(0)?
            .argmax(candle_core::D::Minus1)?
            .to_scalar::<u32>()?;
        pos += 1;
    }
    let dt = t2.elapsed();
    println!(
        "decode {steps} steps: {dt:?} => {:.2} tok/s",
        steps as f64 / dt.as_secs_f64()
    );
    let piece = tok.decode(&[next], false).map_err(|e| e.to_string())?;
    println!("last token: {next} {piece:?}");
    Ok(())
}

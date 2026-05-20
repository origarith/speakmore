use anyhow::{anyhow, bail, Context, Result};
use hound::{SampleFormat, WavReader};
use sherpa_onnx::{OfflineQwen3ASRModelConfig, OfflineRecognizer, OfflineRecognizerConfig};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

const REQUIRED_MODEL_FILES: [&str; 6] = [
    "conv_frontend.onnx",
    "encoder.int8.onnx",
    "decoder.int8.onnx",
    "tokenizer/vocab.json",
    "tokenizer/merges.txt",
    "tokenizer/tokenizer_config.json",
];
const DEFAULT_MAX_NEW_TOKENS: i32 = 384;

fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    if args.len() < 3 || args.len() > 7 {
        bail!(
            "Usage: {} <model-dir> <16k-mono-wav> [language-label] [hotwords] [max-total-len] [max-new-tokens]",
            args.first()
                .map(String::as_str)
                .unwrap_or("qwen3_asr_smoke")
        );
    }

    let model_dir = PathBuf::from(&args[1]);
    let wav_path = PathBuf::from(&args[2]);
    let language = args.get(3).filter(|value| !value.is_empty());
    let hotwords = args.get(4).filter(|value| !value.is_empty());
    let max_total_len = args
        .get(5)
        .map(|value| value.parse::<i32>())
        .transpose()
        .context("Invalid max-total-len")?
        .unwrap_or(1024);
    let max_new_tokens = args
        .get(6)
        .map(|value| value.parse::<i32>())
        .transpose()
        .context("Invalid max-new-tokens")?
        .unwrap_or(DEFAULT_MAX_NEW_TOKENS);

    validate_model_dir(&model_dir)?;
    let (samples, sample_rate) = read_wav(&wav_path)?;
    if sample_rate != 16_000 {
        bail!("Expected 16 kHz audio, got {sample_rate} Hz");
    }

    let audio_seconds = samples.len() as f64 / sample_rate as f64;
    println!("model_dir={}", model_dir.display());
    println!("wav_path={}", wav_path.display());
    println!("audio_seconds={audio_seconds:.3}");
    println!("max_total_len={max_total_len}");
    println!("max_new_tokens={max_new_tokens}");
    println!("rss_before_mb={:.1}", current_rss_mb()?);

    let load_start = Instant::now();
    let recognizer =
        create_recognizer(&model_dir, hotwords.cloned(), max_total_len, max_new_tokens)?;
    let load_elapsed = load_start.elapsed();
    println!("load_ms={:.1}", load_elapsed.as_secs_f64() * 1000.0);
    println!("rss_after_load_mb={:.1}", current_rss_mb()?);

    let decode_start = Instant::now();
    let stream = recognizer.create_stream();
    if let Some(language) = language {
        stream.set_option("language", language);
    }
    stream.accept_waveform(sample_rate as i32, &samples);
    recognizer.decode(&stream);
    let decode_elapsed = decode_start.elapsed();
    println!("decode_ms={:.1}", decode_elapsed.as_secs_f64() * 1000.0);
    println!(
        "rtf={:.3}",
        decode_elapsed.as_secs_f64() / audio_seconds.max(0.001)
    );
    println!("rss_after_decode_mb={:.1}", current_rss_mb()?);

    let result = stream
        .get_result()
        .ok_or_else(|| anyhow!("Qwen3-ASR returned no result"))?;
    println!("text={}", result.text.trim());

    Ok(())
}

fn validate_model_dir(model_dir: &Path) -> Result<()> {
    for file in REQUIRED_MODEL_FILES {
        let path = model_dir.join(file);
        if !path.is_file() {
            bail!("Missing required Qwen3-ASR model file: {}", path.display());
        }
    }

    Ok(())
}

fn create_recognizer(
    model_dir: &Path,
    hotwords: Option<String>,
    max_total_len: i32,
    max_new_tokens: i32,
) -> Result<OfflineRecognizer> {
    let path_string = |path: &Path| path.to_string_lossy().into_owned();

    let mut config = OfflineRecognizerConfig::default();
    config.feat_config.sample_rate = 16_000;
    config.feat_config.feature_dim = 128;
    config.model_config.num_threads = 4;
    config.model_config.provider = Some("cpu".to_string());
    config.model_config.qwen3_asr = OfflineQwen3ASRModelConfig {
        conv_frontend: Some(path_string(&model_dir.join("conv_frontend.onnx"))),
        encoder: Some(path_string(&model_dir.join("encoder.int8.onnx"))),
        decoder: Some(path_string(&model_dir.join("decoder.int8.onnx"))),
        tokenizer: Some(path_string(&model_dir.join("tokenizer"))),
        max_total_len,
        max_new_tokens,
        temperature: 1e-6,
        top_p: 0.8,
        seed: 42,
        hotwords,
    };

    OfflineRecognizer::create(&config)
        .ok_or_else(|| anyhow!("Failed to create Qwen3-ASR ONNX recognizer"))
}

fn read_wav(path: &Path) -> Result<(Vec<f32>, u32)> {
    let mut reader =
        WavReader::open(path).with_context(|| format!("Failed to open WAV: {}", path.display()))?;
    let spec = reader.spec();
    if spec.channels != 1 {
        bail!("Expected mono WAV, got {} channels", spec.channels);
    }

    let samples = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<std::result::Result<Vec<_>, _>>()?,
        SampleFormat::Int if spec.bits_per_sample <= 16 => {
            let max = (1_i64 << (spec.bits_per_sample.saturating_sub(1))) as f32;
            reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| value as f32 / max))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
        SampleFormat::Int => {
            let max = (1_i64 << (spec.bits_per_sample.saturating_sub(1))) as f32;
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|value| value as f32 / max))
                .collect::<std::result::Result<Vec<_>, _>>()?
        }
    };

    Ok((samples, spec.sample_rate))
}

fn current_rss_mb() -> Result<f64> {
    let pid = std::process::id().to_string();
    let output = Command::new("/bin/ps")
        .args(["-o", "rss=", "-p", &pid])
        .output()
        .context("Failed to run ps for RSS measurement")?;
    if !output.status.success() {
        bail!("ps failed while reading RSS");
    }

    let rss_kb = String::from_utf8(output.stdout)?
        .trim()
        .parse::<f64>()
        .context("Failed to parse RSS from ps")?;
    Ok(rss_kb / 1024.0)
}

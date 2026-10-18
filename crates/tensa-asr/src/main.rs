//! `tensa-asr` — runtime для Parakeet/Canary/Nemotron (transcribe.cpp).
//!
//! Отдельный процесс, потому что transcribe.cpp и whisper.cpp тянут ggml.
//! Вход: WAV PCM16 (любая частота, приводим к 16 кГц моно).
//! Использование: `tensa-asr <model.gguf> <audio.wav>`.

use std::path::Path;
use std::process::ExitCode;

use transcribe_cpp::{Model, RunOptions};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: tensa-asr <model.gguf> <audio.wav>");
        return ExitCode::from(2);
    }
    match run(Path::new(&args[1]), Path::new(&args[2])) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run(model: &Path, wav: &Path) -> Result<String, String> {
    if !model.is_file() {
        return Err(format!("модель не найдена: {}", model.display()));
    }
    let bytes = std::fs::read(wav).map_err(|error| format!("чтение WAV: {error}"))?;
    let samples = wav_to_mono_16k(&bytes)?;
    if samples.is_empty() {
        return Err("пустая запись".to_string());
    }
    let mut session = Model::load(model)
        .map_err(|error| format!("загрузка модели: {error}"))?
        .session()
        .map_err(|error| format!("сессия: {error}"))?;
    let transcript = session
        .run(&samples, &RunOptions::default())
        .map_err(|error| format!("распознавание: {error}"))?;
    let text = transcript.text.trim().to_string();
    if text.is_empty() {
        return Err("речь не распознана".to_string());
    }
    Ok(text)
}

fn wav_to_mono_16k(wav: &[u8]) -> Result<Vec<f32>, String> {
    if wav.len() < 44 || &wav[0..4] != b"RIFF" || &wav[8..12] != b"WAVE" {
        return Err("ожидается WAV (RIFF)".to_string());
    }
    let mut position = 12usize;
    let mut format = None;
    let mut data: Option<&[u8]> = None;
    while position + 8 <= wav.len() {
        let id = &wav[position..position + 4];
        let size = u32::from_le_bytes([
            wav[position + 4],
            wav[position + 5],
            wav[position + 6],
            wav[position + 7],
        ]) as usize;
        let body = position + 8;
        if body + size > wav.len() {
            break;
        }
        if id == b"fmt " && size >= 16 {
            let audio_format = u16::from_le_bytes([wav[body], wav[body + 1]]);
            let channels = u16::from_le_bytes([wav[body + 2], wav[body + 3]]);
            let rate =
                u32::from_le_bytes([wav[body + 4], wav[body + 5], wav[body + 6], wav[body + 7]]);
            let bits = u16::from_le_bytes([wav[body + 14], wav[body + 15]]);
            format = Some((audio_format, channels, rate, bits));
        } else if id == b"data" {
            data = Some(&wav[body..body + size]);
            break;
        }
        position = body + size + (size % 2);
    }
    let (audio_format, channels, rate, bits) =
        format.ok_or_else(|| "WAV без fmt-чанка".to_string())?;
    if audio_format != 1 || bits != 16 || channels == 0 {
        return Err("поддерживается WAV PCM16 (моно/стерео)".to_string());
    }
    let data = data.ok_or_else(|| "WAV без данных".to_string())?;
    let mut mono: Vec<f32> = Vec::with_capacity(data.len() / 2 / channels as usize);
    for frame in data.chunks_exact(2 * channels as usize) {
        let mut sum = 0f32;
        for channel in 0..channels as usize {
            let offset = channel * 2;
            let sample = i16::from_le_bytes([frame[offset], frame[offset + 1]]);
            sum += f32::from(sample) / 32768.0;
        }
        mono.push(sum / channels as f32);
    }
    if rate == 16_000 {
        return Ok(mono);
    }
    Ok(resample_linear(&mono, rate, 16_000))
}

fn resample_linear(samples: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == 0 || to == 0 {
        return Vec::new();
    }
    let ratio = f64::from(to) / f64::from(from);
    let length = ((samples.len() as f64) * ratio).floor() as usize;
    let mut out = Vec::with_capacity(length);
    for index in 0..length {
        let position = index as f64 / ratio;
        let base = position.floor() as usize;
        let frac = (position - base as f64) as f32;
        let a = samples.get(base).copied().unwrap_or(0.0);
        let b = samples.get(base + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

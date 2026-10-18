//! `tensa-whisper` — отдельный процесс локального распознавания речи.
//!
//! Вынесен из `tensa-server`, потому что llama.cpp и whisper.cpp содержат
//! одинаковые символы ggml: в одном exe они не линкуются. Сервер запускает
//! helper на каждый запрос: `tensa-whisper <model.ggml> <audio.wav>`.
//!
//! Вход: WAV PCM16 (моно/стерео, любая частота — ресемплим в 16 кГц).
//! Выход: распознанный текст в stdout; ошибки — в stderr, код 1.

use std::path::Path;
use std::process::ExitCode;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("usage: tensa-whisper <model> <audio.wav>");
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

/// Распознать файл и вернуть текст.
fn run(model: &Path, wav: &Path) -> Result<String, String> {
    if !model.is_file() {
        return Err(format!("модель не найдена: {}", model.display()));
    }
    let bytes = std::fs::read(wav).map_err(|error| format!("чтение WAV: {error}"))?;
    let samples = wav_to_mono_16k(&bytes)?;
    if samples.is_empty() {
        return Err("пустая запись".to_string());
    }

    let context = WhisperContext::new_with_params(
        model.to_string_lossy().as_ref(),
        WhisperContextParameters::default(),
    )
    .map_err(|error| format!("загрузка модели: {error}"))?;
    let mut state = context
        .create_state()
        .map_err(|error| format!("состояние: {error}"))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_language(Some("ru"));
    params.set_n_threads(4);
    params.set_no_speech_thold(0.6);
    params.set_suppress_blank(true);
    state
        .full(params, &samples)
        .map_err(|error| format!("распознавание: {error}"))?;

    let mut text = String::new();
    for index in 0..state.full_n_segments() {
        if let Some(segment) = state.get_segment(index) {
            if let Ok(part) = segment.to_str() {
                text.push_str(part);
            }
        }
    }
    let text = clean_transcript(&text);
    if text.is_empty() {
        return Err("речь не распознана".to_string());
    }
    Ok(text)
}

/// Убрать неречевые пометки whisper (`[музыка]`, `(смех)`, `♪`).
fn clean_transcript(raw: &str) -> String {
    let mut words = Vec::new();
    for word in raw.split_whitespace() {
        let trimmed = word.trim_matches(|c: char| c.is_ascii_punctuation() || c == '♪');
        let annotated = (word.starts_with('[') && word.ends_with(']'))
            || (word.starts_with('(') && word.ends_with(')'));
        if annotated || trimmed.is_empty() {
            continue;
        }
        words.push(word);
    }
    words.join(" ").trim().to_string()
}

/// Разобрать WAV и вернуть моно 16 кГц f32.
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
    resample_linear(&mono, rate, 16_000)
}

/// Линейный ресемплинг (достаточно для речи).
fn resample_linear(samples: &[f32], from: u32, to: u32) -> Result<Vec<f32>, String> {
    if from == 0 || to == 0 {
        return Err("некорректная частота".to_string());
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
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WAV PCM16 из сэмплов.
    fn wav(samples: &[i16], rate: u32, channels: u16) -> Vec<u8> {
        let data_len = (samples.len() * 2) as u32;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&rate.to_le_bytes());
        out.extend_from_slice(&(rate * 2 * u32::from(channels)).to_le_bytes());
        out.extend_from_slice(&(2 * channels).to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for sample in samples {
            out.extend_from_slice(&sample.to_le_bytes());
        }
        out
    }

    #[test]
    fn parses_stereo_and_downmixes() {
        let samples = [1000i16, -1000, 2000, -2000];
        let parsed = wav_to_mono_16k(&wav(&samples, 16_000, 2)).expect("wav");
        assert_eq!(parsed.len(), 2);
        assert!(parsed[0].abs() < 1e-6);
    }

    #[test]
    fn resamples_to_16k() {
        let samples: Vec<i16> = (0..4800).map(|index| (index % 100) as i16).collect();
        let parsed = wav_to_mono_16k(&wav(&samples, 48_000, 1)).expect("wav");
        assert!((parsed.len() as i64 - 1600).abs() <= 2);
    }

    #[test]
    fn rejects_non_wav() {
        assert!(wav_to_mono_16k(b"not a wav").is_err());
    }
}

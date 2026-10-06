use crate::interfaces::probe::{Subtitle, SubtitleProbe};
use serde_json::Value;
use std::process::Command;

pub struct Ffprobe;

impl SubtitleProbe for Ffprobe {
    fn get_subtitle_streams(&self, input: &str) -> Result<Vec<Subtitle>, String> {
        get_subtitle_streams(input)
    }
}

fn get_subtitle_streams(input: &str) -> Result<Vec<Subtitle>, String> {
    let ffprobe_output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
            input,
        ])
        .output()
        .map_err(|error| format!("Falha ao executar ffprobe: {error}"))?;

    if !ffprobe_output.status.success() {
        let ffprobe_stderr = String::from_utf8_lossy(&ffprobe_output.stderr);
        return Err(format!(
            "Comando ffprobe terminou com erro: {}\nstderr do ffprobe:\n{}",
            ffprobe_output.status, ffprobe_stderr
        ));
    }

    let ffprobe_stdout = String::from_utf8_lossy(&ffprobe_output.stdout);
    let ffprobe_json: Value = serde_json::from_str(&ffprobe_stdout)
        .map_err(|error| format!("Falha ao converter a saida do ffprobe em JSON: {error}"))?;

    let subtitle_streams = ffprobe_json
        .get("streams")
        .and_then(Value::as_array)
        .map(|streams| {
            streams
                .iter()
                .filter(|stream| {
                    stream.get("codec_type").and_then(Value::as_str) == Some("subtitle")
                })
                .map(|stream| {
                    let index = stream.get("index").and_then(Value::as_i64).unwrap_or(-1);
                    let language = stream
                        .get("tags")
                        .and_then(|tags| tags.get("language"))
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let is_sdh = stream
                        .get("disposition")
                        .and_then(|disposition| disposition.get("hearing_impaired"))
                        .and_then(Value::as_i64)
                        .map(|value| value == 1)
                        .unwrap_or(false);

                    Subtitle {
                        index,
                        language,
                        is_sdh,
                    }
                })
                .collect::<Vec<Subtitle>>()
        })
        .unwrap_or_default();

    Ok(subtitle_streams)
}

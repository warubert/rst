use crate::interfaces::probe::{Subtitle, SubtitleProbe};
use rust_i18n::t;
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
        .map_err(|error| t!("errors.ffprobe_start", error = error).to_string())?;

    if !ffprobe_output.status.success() {
        let ffprobe_stderr = String::from_utf8_lossy(&ffprobe_output.stderr);
        return Err(t!(
            "errors.ffprobe_status",
            status = ffprobe_output.status,
            stderr = ffprobe_stderr
        )
        .to_string());
    }

    let ffprobe_stdout = String::from_utf8_lossy(&ffprobe_output.stdout);
    parse_subtitle_streams(&ffprobe_stdout)
}

fn parse_subtitle_streams(ffprobe_stdout: &str) -> Result<Vec<Subtitle>, String> {
    let ffprobe_json: Value = serde_json::from_str(ffprobe_stdout)
        .map_err(|error| t!("errors.ffprobe_json", error = error).to_string())?;

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
                        .map(str::to_string)
                        .unwrap_or_else(|| t!("tui.unknown_language").to_string());
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

#[cfg(test)]
mod tests {
    use super::parse_subtitle_streams;

    #[test]
    fn parses_subtitle_streams_and_skips_other_streams() {
        let output = r#"{
            "streams": [
                {"index": 0, "codec_type": "video"},
                {
                    "index": 3,
                    "codec_type": "subtitle",
                    "tags": {"language": "pt-BR"},
                    "disposition": {"hearing_impaired": 1}
                },
                {
                    "index": 4,
                    "codec_type": "subtitle",
                    "tags": {"language": "en"},
                    "disposition": {"hearing_impaired": 0}
                }
            ]
        }"#;

        let subtitles = parse_subtitle_streams(output).unwrap();

        assert_eq!(subtitles.len(), 2);
        assert_eq!(subtitles[0].index, 3);
        assert_eq!(subtitles[0].language, "pt-BR");
        assert!(subtitles[0].is_sdh);
        assert_eq!(subtitles[1].index, 4);
        assert_eq!(subtitles[1].language, "en");
        assert!(!subtitles[1].is_sdh);
    }

    #[test]
    fn uses_defaults_when_subtitle_metadata_is_missing() {
        let subtitles = parse_subtitle_streams(
            r#"{"streams":[{"codec_type":"subtitle","tags":{},"disposition":{}}]}"#,
        )
        .unwrap();

        assert_eq!(subtitles.len(), 1);
        assert_eq!(subtitles[0].index, -1);
        assert!(!subtitles[0].language.is_empty());
        assert!(!subtitles[0].is_sdh);
    }

    #[test]
    fn returns_empty_list_when_no_subtitles_exist() {
        assert!(
            parse_subtitle_streams(r#"{"streams":[{"codec_type":"video"}]}"#)
                .unwrap()
                .is_empty()
        );
        assert!(
            parse_subtitle_streams(r#"{"format":{}}"#)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn reports_invalid_json() {
        assert!(parse_subtitle_streams("not json").is_err());
    }
}

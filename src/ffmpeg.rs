use crate::interfaces::ffmpeg::SubtitleExtractor;
use rust_i18n::t;
use std::process::Command;

pub struct Ffmpeg;

impl SubtitleExtractor for Ffmpeg {
    fn extract_subtitle(
        &self,
        input: &str,
        subtitle_index: i64,
        output_srt: &str,
    ) -> Result<(), String> {
        extract_subtitle(input, subtitle_index, output_srt)
    }
}

fn extract_subtitle(input: &str, subtitle_index: i64, output_srt: &str) -> Result<(), String> {
    let ffmpeg_output = Command::new("ffmpeg")
        .args(extraction_args(input, subtitle_index, output_srt))
        .output()
        .map_err(|error| t!("errors.ffmpeg_start", error = error).to_string())?;

    if !ffmpeg_output.status.success() {
        let ffmpeg_stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
        return Err(t!(
            "errors.ffmpeg_status",
            status = ffmpeg_output.status,
            stderr = ffmpeg_stderr
        )
        .to_string());
    }

    Ok(())
}

fn extraction_args(input: &str, subtitle_index: i64, output_srt: &str) -> [String; 5] {
    [
        "-i".to_string(),
        input.to_string(),
        "-map".to_string(),
        format!("0:{subtitle_index}"),
        output_srt.to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::extraction_args;

    #[test]
    fn maps_selected_stream_and_output_path() {
        assert_eq!(
            extraction_args("episode file.mkv", 7, "episode.srt"),
            ["-i", "episode file.mkv", "-map", "0:7", "episode.srt"]
        );
    }

    #[test]
    fn maps_negative_stream_index_without_changing_the_input() {
        assert_eq!(
            extraction_args("input.mkv", -1, "output.srt"),
            ["-i", "input.mkv", "-map", "0:-1", "output.srt"]
        );
    }
}

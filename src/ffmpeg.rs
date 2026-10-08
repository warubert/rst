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
    let map_argument = format!("0:{subtitle_index}");

    let ffmpeg_output = Command::new("ffmpeg")
        .args(["-i", input, "-map", &map_argument, output_srt])
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

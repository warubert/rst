use crate::interfaces::ffmpeg::SubtitleExtractor;
use crate::srt::serialize;

mod cues;
mod matroska;
mod mp4;

use rust_i18n::t;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

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

    fn extract_subtitle_with_progress(
        &self,
        input: &str,
        subtitle_index: i64,
        output_srt: &str,
        on_progress: &mut dyn FnMut(u8),
    ) -> Result<(), String> {
        extract_subtitle_reporting_progress(input, subtitle_index, output_srt, on_progress)
    }
}

fn extract_subtitle(input: &str, subtitle_index: i64, output_srt: &str) -> Result<(), String> {
    extract_subtitle_reporting_progress(input, subtitle_index, output_srt, &mut |_| {})
}

fn extract_subtitle_reporting_progress(
    input: &str,
    subtitle_index: i64,
    output_srt: &str,
    on_progress: &mut dyn FnMut(u8),
) -> Result<(), String> {
    on_progress(0);
    let mut file = File::open(input)
        .map_err(|error| t!("errors.media_open", input = input, error = error).to_string())?;
    let mut signature = [0; 8];
    let signature_len = file
        .read(&mut signature)
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?;
    file.seek(SeekFrom::Start(0))
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?;
    let extension = Path::new(input)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    let cues = if (signature_len >= 4 && signature[..4] == [0x1a, 0x45, 0xdf, 0xa3])
        || matches!(extension.as_str(), "mkv" | "webm")
    {
        matroska::extract_subtitle(file, subtitle_index, on_progress)?
    } else if (signature_len >= 8 && &signature[4..8] == b"ftyp")
        || matches!(extension.as_str(), "mp4" | "mov")
    {
        mp4::extract_subtitle(file, input, subtitle_index, on_progress)?
    } else {
        return Err(t!("errors.unsupported_media", input = input).to_string());
    };

    cues::write_srt(output_srt, &serialize(&cues))?;
    on_progress(100);
    Ok(())
}

#[cfg(test)]
#[path = "../tests/ffmpeg.rs"]
mod tests;

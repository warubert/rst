use crate::interfaces::probe::{Subtitle, SubtitleProbe};

mod mp4_metadata;
use rust_i18n::t;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    path::Path,
};

pub struct Ffprobe;

impl SubtitleProbe for Ffprobe {
    fn get_subtitle_streams(&self, input: &str) -> Result<Vec<Subtitle>, String> {
        get_subtitle_streams(input)
    }
}

fn get_subtitle_streams(input: &str) -> Result<Vec<Subtitle>, String> {
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

    if (signature_len >= 4 && signature[..4] == [0x1a, 0x45, 0xdf, 0xa3])
        || matches!(extension.as_str(), "mkv" | "webm")
    {
        get_matroska_subtitle_streams(file)
    } else if (signature_len >= 8 && &signature[4..8] == b"ftyp")
        || matches!(extension.as_str(), "mp4" | "mov")
    {
        get_mp4_subtitle_streams(file, input)
    } else {
        Err(t!("errors.unsupported_media", input = input).to_string())
    }
}

fn get_matroska_subtitle_streams(file: File) -> Result<Vec<Subtitle>, String> {
    let container = matroska::Matroska::open(file).map_err(|error| {
        t!("errors.media_parse", container = "Matroska", error = error).to_string()
    })?;

    Ok(container
        .tracks
        .iter()
        .enumerate()
        .filter(|(_, track)| track.is_subtitle())
        .map(|(index, track)| Subtitle {
            index: i64::try_from(index).unwrap_or(-1),
            language: track
                .language
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| t!("tui.unknown_language").to_string()),
            is_sdh: track.hearing_impaired.unwrap_or(false),
        })
        .collect())
}

fn get_mp4_subtitle_streams(file: File, input: &str) -> Result<Vec<Subtitle>, String> {
    let file_size = file
        .metadata()
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?
        .len();
    let container = mp4::Mp4Reader::read_header(BufReader::new(file), file_size)
        .map_err(|error| t!("errors.media_parse", container = "MP4", error = error).to_string())?;
    let hearing_impaired_flags = mp4_metadata::mp4_hearing_impaired_flags(input)?;

    if hearing_impaired_flags.len() != container.moov.traks.len() {
        return Err(t!(
            "errors.media_parse",
            container = "MP4",
            error = "track metadata count does not match"
        )
        .to_string());
    }

    Ok(container
        .moov
        .traks
        .iter()
        .zip(hearing_impaired_flags)
        .enumerate()
        .filter(|(_, (track, _))| {
            is_mp4_subtitle_handler(&track.mdia.hdlr.handler_type.to_string())
        })
        .map(|(index, (track, is_sdh))| Subtitle {
            index: i64::try_from(index).unwrap_or(-1),
            language: track.mdia.mdhd.language.clone(),
            is_sdh,
        })
        .collect())
}

fn is_mp4_subtitle_handler(handler: &str) -> bool {
    matches!(handler, "sbtl" | "subt" | "text" | "clcp")
}

#[cfg(test)]
#[path = "../tests/ffprobe.rs"]
mod tests;

use super::cues::{
    ExtractedCue, frames_to_cues, progress_percent, selected_track_index,
    track_ticks_to_milliseconds,
};
use crate::srt::Cue;
use rust_i18n::t;
use std::{fs::File, io::BufReader};

pub(super) fn extract_subtitle(
    file: File,
    input: &str,
    subtitle_index: i64,
    on_progress: &mut dyn FnMut(u8),
) -> Result<Vec<Cue>, String> {
    let file_size = file
        .metadata()
        .map_err(|error| t!("errors.media_read", input = input, error = error).to_string())?
        .len();
    let mut container = mp4::Mp4Reader::read_header(BufReader::new(file), file_size)
        .map_err(|error| t!("errors.media_parse", container = "MP4", error = error).to_string())?;
    let track_index = selected_track_index(subtitle_index, container.moov.traks.len())?;
    let stream = &container.moov.traks[track_index];
    let handler = stream.mdia.hdlr.handler_type.to_string();
    if !is_mp4_subtitle_handler(&handler) {
        return Err(t!("errors.subtitle_track_invalid", index = subtitle_index).to_string());
    }

    let track_id = stream.tkhd.track_id;
    let track = container
        .tracks()
        .get(&track_id)
        .ok_or_else(|| t!("errors.subtitle_track_invalid", index = subtitle_index).to_string())?;
    if track.media_type().ok() != Some(mp4::MediaType::TTXT) {
        return Err(t!("errors.subtitle_codec_unsupported", codec = handler).to_string());
    }
    let timescale = u64::from(track.timescale());
    if timescale == 0 {
        return Err(t!(
            "errors.media_parse",
            container = "MP4",
            error = "subtitle track has a zero timescale"
        )
        .to_string());
    }
    let sample_count = container
        .sample_count(track_id)
        .map_err(|error| t!("errors.media_parse", container = "MP4", error = error).to_string())?;

    let mut frames = Vec::new();
    for sample_id in 1..=sample_count {
        if sample_count > 0 {
            let percent = progress_percent(u64::from(sample_id), u64::from(sample_count)).min(99);
            on_progress(percent);
        }
        let Some(sample) = container
            .read_sample(track_id, sample_id)
            .map_err(|error| {
                t!("errors.media_parse", container = "MP4", error = error).to_string()
            })?
        else {
            continue;
        };
        let text = decode_tx3g_sample(sample.bytes.as_ref())?;
        if text.trim().is_empty() {
            continue;
        }
        let presentation_time = if sample.rendering_offset < 0 {
            sample
                .start_time
                .saturating_sub(u64::from(sample.rendering_offset.unsigned_abs()))
        } else {
            sample
                .start_time
                .saturating_add(u64::try_from(sample.rendering_offset).unwrap_or(0))
        };
        frames.push(ExtractedCue {
            start_ms: track_ticks_to_milliseconds(presentation_time, timescale),
            duration_ms: Some(track_ticks_to_milliseconds(
                u64::from(sample.duration),
                timescale,
            )),
            text,
        });
    }

    Ok(frames_to_cues(frames))
}

pub(super) fn is_mp4_subtitle_handler(handler: &str) -> bool {
    matches!(handler, "sbtl" | "subt" | "text" | "clcp")
}

pub(super) fn decode_tx3g_sample(data: &[u8]) -> Result<String, String> {
    let length = data
        .get(..2)
        .and_then(|prefix| <[u8; 2]>::try_from(prefix).ok())
        .map(u16::from_be_bytes)
        .ok_or_else(|| {
            t!(
                "errors.subtitle_decode",
                error = "truncated MP4 text sample"
            )
            .to_string()
        })? as usize;
    let text = data.get(2..2 + length).ok_or_else(|| {
        t!(
            "errors.subtitle_decode",
            error = "truncated MP4 text sample"
        )
        .to_string()
    })?;

    if let Some(utf16) = text.strip_prefix(&[0xfe, 0xff]) {
        return decode_utf16(utf16, true);
    }
    if let Some(utf16) = text.strip_prefix(&[0xff, 0xfe]) {
        return decode_utf16(utf16, false);
    }
    std::str::from_utf8(text)
        .map(str::to_string)
        .map_err(|error| t!("errors.subtitle_decode", error = error).to_string())
}

pub(super) fn decode_utf16(data: &[u8], big_endian: bool) -> Result<String, String> {
    if data.len() & 1 != 0 {
        return Err(t!(
            "errors.subtitle_decode",
            error = "invalid UTF-16 text sample"
        )
        .to_string());
    }
    let words = data.chunks(2).map(|bytes| {
        let pair = [bytes[0], bytes[1]];
        if big_endian {
            u16::from_be_bytes(pair)
        } else {
            u16::from_le_bytes(pair)
        }
    });
    char::decode_utf16(words)
        .collect::<Result<String, _>>()
        .map_err(|error| t!("errors.subtitle_decode", error = error).to_string())
}

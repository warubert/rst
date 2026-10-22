use super::cues::{
    ExtractedCue, frames_to_cues, matroska_progress_percent, matroska_ticks_to_milliseconds,
    selected_track_index,
};
use crate::srt::Cue;
use rust_i18n::t;
use std::fs::File;

pub(super) fn extract_subtitle(
    file: File,
    subtitle_index: i64,
    on_progress: &mut dyn FnMut(u8),
) -> Result<Vec<Cue>, String> {
    let mut container = matroska_demuxer::MatroskaFile::open(file).map_err(|error| {
        t!("errors.media_parse", container = "Matroska", error = error).to_string()
    })?;
    let track_index = selected_track_index(subtitle_index, container.tracks().len())?;
    let selected_track = &container.tracks()[track_index];
    if selected_track.track_type() != matroska_demuxer::TrackType::Subtitle {
        return Err(t!("errors.subtitle_track_invalid", index = subtitle_index).to_string());
    }

    let codec_id = selected_track.codec_id().to_string();
    let codec_private = selected_track.codec_private().map(ToOwned::to_owned);
    if !is_matroska_text_codec(&codec_id) {
        return Err(t!("errors.subtitle_codec_unsupported", codec = codec_id).to_string());
    }

    let track_number = selected_track.track_number().get();
    let timestamp_scale = container.info().timestamp_scale().get();
    let duration = container
        .info()
        .duration()
        .filter(|duration| duration.is_finite() && *duration > 0.0);
    let mut frames = Vec::new();
    let mut frame = matroska_demuxer::Frame::default();
    while container.next_frame(&mut frame).map_err(|error| {
        t!("errors.media_parse", container = "Matroska", error = error).to_string()
    })? {
        if let Some(percent) = matroska_progress_percent(frame.timestamp, duration) {
            on_progress(percent);
        }
        if frame.track != track_number {
            continue;
        }

        let text = decode_matroska_text(&codec_id, codec_private.as_deref(), &frame.data)?;
        if text.trim().is_empty() {
            continue;
        }
        frames.push(ExtractedCue {
            start_ms: matroska_ticks_to_milliseconds(frame.timestamp, timestamp_scale),
            duration_ms: frame
                .duration
                .map(|duration| matroska_ticks_to_milliseconds(duration, timestamp_scale)),
            text,
        });
    }

    Ok(frames_to_cues(frames))
}

pub(super) fn is_matroska_text_codec(codec: &str) -> bool {
    matches!(
        codec,
        "S_TEXT/UTF8"
            | "S_TEXT/ASS"
            | "S_TEXT/SSA"
            | "S_TEXT/WEBVTT"
            | "D_WEBVTT/SUBTITLES"
            | "D_WEBVTT/CAPTIONS"
            | "D_WEBVTT/DESCRIPTIONS"
            | "D_WEBVTT/METADATA"
    )
}

pub(super) fn decode_matroska_text(
    codec: &str,
    codec_private: Option<&[u8]>,
    data: &[u8],
) -> Result<String, String> {
    let text = std::str::from_utf8(data)
        .map_err(|error| t!("errors.subtitle_decode", error = error).to_string())?
        .trim_matches('\0')
        .trim_end_matches(['\r', '\n']);

    match codec {
        "S_TEXT/UTF8" => Ok(text.to_string()),
        "S_TEXT/ASS" | "S_TEXT/SSA" => {
            let text = ass_text_payload(codec_private, text);
            Ok(ass_to_srt_text(text))
        }
        _ => Ok(webvtt_text_payload(text).to_string()),
    }
}

pub(super) fn ass_text_payload<'a>(codec_private: Option<&[u8]>, data: &'a str) -> &'a str {
    let is_dialogue = data.starts_with("Dialogue:") || data.starts_with("Comment:");
    let data = data
        .strip_prefix("Dialogue:")
        .or_else(|| data.strip_prefix("Comment:"))
        .map(str::trim_start)
        .unwrap_or(data);
    let text_field = codec_private
        .and_then(|header| std::str::from_utf8(header).ok())
        .and_then(|header| {
            header.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("Format:")?
                    .split(',')
                    .map(str::trim)
                    .position(|field| field.eq_ignore_ascii_case("Text"))
            })
        })
        .unwrap_or(if is_dialogue { 9 } else { 8 });

    data.splitn(text_field + 1, ',')
        .nth(text_field)
        .unwrap_or(data)
}

pub(super) fn ass_to_srt_text(text: &str) -> String {
    let mut output = String::new();
    let mut chars = text.chars().peekable();
    let mut italic = false;
    let mut bold = false;
    let mut underline = false;

    while let Some(character) = chars.next() {
        match character {
            '{' => {
                let mut override_tags = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    override_tags.push(next);
                }
                if !closed {
                    output.push('{');
                    output.push_str(&override_tags);
                    break;
                }
                for (tag, open, close, state) in [
                    ("\\i1", "<i>", "</i>", &mut italic),
                    ("\\b1", "<b>", "</b>", &mut bold),
                    ("\\u1", "<u>", "</u>", &mut underline),
                ] {
                    if override_tags.contains(tag) && !*state {
                        output.push_str(open);
                        *state = true;
                    }
                    let off_tag = tag.replace('1', "0");
                    if override_tags.contains(&off_tag) && *state {
                        output.push_str(close);
                        *state = false;
                    }
                }
                for alignment in 1..=9 {
                    let alignment_tag = format!("\\an{alignment}");
                    if override_tags.contains(&alignment_tag) {
                        output.push_str(&format!("{{\\an{alignment}}}"));
                        break;
                    }
                }
            }
            '\\' => match chars.peek() {
                Some('N' | 'n') => {
                    output.push('\n');
                    chars.next();
                }
                Some('h') => {
                    output.push(' ');
                    chars.next();
                }
                _ => output.push(character),
            },
            _ => output.push(character),
        }
    }

    if underline {
        output.push_str("</u>");
    }
    if bold {
        output.push_str("</b>");
    }
    if italic {
        output.push_str("</i>");
    }
    output
}

pub(super) fn webvtt_text_payload(data: &str) -> &str {
    let mut fields = data.split('\0');
    match (fields.next(), fields.next(), fields.next()) {
        (Some(_cue_id), Some(_settings), Some(payload)) => payload,
        _ => data,
    }
}

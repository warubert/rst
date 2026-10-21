use crate::interfaces::ffmpeg::SubtitleExtractor;
use crate::srt::{Cue, serialize};
use rust_i18n::t;
use std::{
    fs::{File, OpenOptions},
    io::{BufReader, Read, Seek, SeekFrom, Write},
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
        extract_matroska_subtitle(file, subtitle_index, on_progress)?
    } else if (signature_len >= 8 && &signature[4..8] == b"ftyp")
        || matches!(extension.as_str(), "mp4" | "mov")
    {
        extract_mp4_subtitle(file, input, subtitle_index, on_progress)?
    } else {
        return Err(t!("errors.unsupported_media", input = input).to_string());
    };

    write_srt(output_srt, &serialize(&cues))?;
    on_progress(100);
    Ok(())
}

#[derive(Debug)]
struct ExtractedCue {
    start_ms: u64,
    duration_ms: Option<u64>,
    text: String,
}

fn extract_matroska_subtitle(
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

fn extract_mp4_subtitle(
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

fn progress_percent(completed: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }

    (u128::from(completed) * 100 / u128::from(total)).min(100) as u8
}

fn matroska_progress_percent(timestamp: u64, duration: Option<f64>) -> Option<u8> {
    duration
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| {
            ((timestamp as f64 / duration) * 100.0)
                .floor()
                .clamp(0.0, 99.0) as u8
        })
}

fn selected_track_index(subtitle_index: i64, track_count: usize) -> Result<usize, String> {
    let index = usize::try_from(subtitle_index)
        .ok()
        .filter(|index| *index < track_count)
        .ok_or_else(|| t!("errors.subtitle_track_invalid", index = subtitle_index).to_string())?;
    Ok(index)
}

fn is_matroska_text_codec(codec: &str) -> bool {
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

fn is_mp4_subtitle_handler(handler: &str) -> bool {
    matches!(handler, "sbtl" | "subt" | "text" | "clcp")
}

fn decode_matroska_text(
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

fn ass_text_payload<'a>(codec_private: Option<&[u8]>, data: &'a str) -> &'a str {
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

fn ass_to_srt_text(text: &str) -> String {
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

fn webvtt_text_payload(data: &str) -> &str {
    let mut fields = data.split('\0');
    match (fields.next(), fields.next(), fields.next()) {
        (Some(_cue_id), Some(_settings), Some(payload)) => payload,
        _ => data,
    }
}

fn decode_tx3g_sample(data: &[u8]) -> Result<String, String> {
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

fn decode_utf16(data: &[u8], big_endian: bool) -> Result<String, String> {
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

fn matroska_ticks_to_milliseconds(ticks: u64, scale: u64) -> u64 {
    let nanoseconds = u128::from(ticks).saturating_mul(u128::from(scale));
    u64::try_from(nanoseconds.saturating_add(500_000) / 1_000_000).unwrap_or(u64::MAX)
}

fn track_ticks_to_milliseconds(ticks: u64, timescale: u64) -> u64 {
    let milliseconds = u128::from(ticks).saturating_mul(1000);
    u64::try_from(milliseconds.saturating_add(u128::from(timescale / 2)) / u128::from(timescale))
        .unwrap_or(u64::MAX)
}

fn frames_to_cues(mut frames: Vec<ExtractedCue>) -> Vec<Cue> {
    frames.sort_by_key(|frame| frame.start_ms);
    let starts = frames
        .iter()
        .map(|frame| frame.start_ms)
        .collect::<Vec<_>>();
    frames
        .into_iter()
        .enumerate()
        .map(|(index, frame)| {
            let inferred_duration = starts
                .get(index + 1)
                .filter(|next_start| **next_start > frame.start_ms)
                .map(|next_start| next_start - frame.start_ms)
                .unwrap_or(2000);
            let duration = frame.duration_ms.filter(|duration| *duration > 0);
            let end_ms = frame
                .start_ms
                .saturating_add(duration.unwrap_or(inferred_duration));

            Cue {
                index: (index + 1).to_string(),
                timing: format!(
                    "{} --> {}",
                    format_srt_timestamp(frame.start_ms),
                    format_srt_timestamp(end_ms)
                ),
                text: frame.text,
            }
        })
        .collect()
}

fn format_srt_timestamp(milliseconds: u64) -> String {
    let hours = milliseconds / 3_600_000;
    let minutes = milliseconds / 60_000 % 60;
    let seconds = milliseconds / 1000 % 60;
    let fractional = milliseconds % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{fractional:03}")
}

fn write_srt(output: &str, content: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| t!("errors.subtitle_output", path = output, error = error).to_string())?;
    file.write_all(content.as_bytes())
        .map_err(|error| t!("errors.subtitle_output", path = output, error = error).to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        ExtractedCue, ass_text_payload, ass_to_srt_text, decode_tx3g_sample, format_srt_timestamp,
        frames_to_cues, is_matroska_text_codec, matroska_progress_percent, progress_percent,
        selected_track_index,
    };

    #[test]
    fn computes_extraction_progress_without_reporting_completion_early() {
        assert_eq!(progress_percent(1, 4), 25);
        assert_eq!(progress_percent(0, 0), 0);
        assert_eq!(matroska_progress_percent(25, Some(100.0)), Some(25));
        assert_eq!(matroska_progress_percent(100, Some(100.0)), Some(99));
        assert_eq!(matroska_progress_percent(25, None), None);
        assert_eq!(matroska_progress_percent(25, Some(0.0)), None);
    }

    #[test]
    fn selects_global_stream_indices_and_rejects_invalid_indices() {
        assert_eq!(selected_track_index(2, 4).unwrap(), 2);
        assert!(selected_track_index(-1, 4).is_err());
        assert!(selected_track_index(4, 4).is_err());
    }

    #[test]
    fn supports_common_text_subtitle_codecs() {
        for codec in [
            "S_TEXT/UTF8",
            "S_TEXT/ASS",
            "S_TEXT/SSA",
            "S_TEXT/WEBVTT",
            "D_WEBVTT/SUBTITLES",
        ] {
            assert!(is_matroska_text_codec(codec));
        }
        assert!(!is_matroska_text_codec("S_HDMV/PGS"));
    }

    #[test]
    fn converts_ass_text_and_keeps_basic_formatting() {
        let codec_private = b"[Events]\nFormat: ReadOrder, Layer, Style, Name, MarginL, MarginR, MarginV, Effect, Text";
        assert_eq!(
            ass_text_payload(
                Some(codec_private),
                "0,0,Default,,0,0,0,,{\\i1}Hi{\\i0}\\Nthere"
            ),
            "{\\i1}Hi{\\i0}\\Nthere"
        );
        assert_eq!(
            ass_to_srt_text("{\\an8}{\\b1}Hi{\\b0}\\Nthere"),
            "{\\an8}<b>Hi</b>\nthere"
        );
    }

    #[test]
    fn decodes_utf8_mp4_text_samples_and_rejects_truncated_samples() {
        let sample = [0, 5, b'h', b'e', b'l', b'l', b'o'];
        assert_eq!(decode_tx3g_sample(&sample).unwrap(), "hello");
        assert!(decode_tx3g_sample(&[0, 2, b'h']).is_err());
    }

    #[test]
    fn serializes_cues_with_container_timestamps_and_infers_missing_ends() {
        let cues = frames_to_cues(vec![
            ExtractedCue {
                start_ms: 1500,
                duration_ms: None,
                text: "first".to_string(),
            },
            ExtractedCue {
                start_ms: 2500,
                duration_ms: Some(1250),
                text: "second".to_string(),
            },
        ]);

        assert_eq!(cues[0].timing, "00:00:01,500 --> 00:00:02,500");
        assert_eq!(cues[1].timing, "00:00:02,500 --> 00:00:03,750");
        assert_eq!(format_srt_timestamp(3_600_001), "01:00:00,001");
    }
}

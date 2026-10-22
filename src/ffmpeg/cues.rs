use crate::srt::Cue;
use rust_i18n::t;
use std::{fs::OpenOptions, io::Write};

#[derive(Debug)]
pub(super) struct ExtractedCue {
    pub(super) start_ms: u64,
    pub(super) duration_ms: Option<u64>,
    pub(super) text: String,
}

pub(super) fn progress_percent(completed: u64, total: u64) -> u8 {
    if total == 0 {
        return 0;
    }

    (u128::from(completed) * 100 / u128::from(total)).min(100) as u8
}

pub(super) fn matroska_progress_percent(timestamp: u64, duration: Option<f64>) -> Option<u8> {
    duration
        .filter(|duration| duration.is_finite() && *duration > 0.0)
        .map(|duration| {
            ((timestamp as f64 / duration) * 100.0)
                .floor()
                .clamp(0.0, 99.0) as u8
        })
}

pub(super) fn selected_track_index(
    subtitle_index: i64,
    track_count: usize,
) -> Result<usize, String> {
    let index = usize::try_from(subtitle_index)
        .ok()
        .filter(|index| *index < track_count)
        .ok_or_else(|| t!("errors.subtitle_track_invalid", index = subtitle_index).to_string())?;
    Ok(index)
}

pub(super) fn matroska_ticks_to_milliseconds(ticks: u64, scale: u64) -> u64 {
    let nanoseconds = u128::from(ticks).saturating_mul(u128::from(scale));
    u64::try_from(nanoseconds.saturating_add(500_000) / 1_000_000).unwrap_or(u64::MAX)
}

pub(super) fn track_ticks_to_milliseconds(ticks: u64, timescale: u64) -> u64 {
    let milliseconds = u128::from(ticks).saturating_mul(1000);
    u64::try_from(milliseconds.saturating_add(u128::from(timescale / 2)) / u128::from(timescale))
        .unwrap_or(u64::MAX)
}

pub(super) fn frames_to_cues(mut frames: Vec<ExtractedCue>) -> Vec<Cue> {
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

pub(super) fn format_srt_timestamp(milliseconds: u64) -> String {
    let hours = milliseconds / 3_600_000;
    let minutes = milliseconds / 60_000 % 60;
    let seconds = milliseconds / 1000 % 60;
    let fractional = milliseconds % 1000;
    format!("{hours:02}:{minutes:02}:{seconds:02},{fractional:03}")
}

pub(super) fn write_srt(output: &str, content: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)
        .map_err(|error| t!("errors.subtitle_output", path = output, error = error).to_string())?;
    file.write_all(content.as_bytes())
        .map_err(|error| t!("errors.subtitle_output", path = output, error = error).to_string())
}

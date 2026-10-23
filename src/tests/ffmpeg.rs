use super::{
    cues::{
        ExtractedCue, format_srt_timestamp, frames_to_cues, matroska_progress_percent,
        progress_percent, selected_track_index,
    },
    matroska::{ass_text_payload, ass_to_srt_text, is_matroska_text_codec},
    mp4::decode_tx3g_sample,
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
    let codec_private =
        b"[Events]\nFormat: ReadOrder, Layer, Style, Name, MarginL, MarginR, MarginV, Effect, Text";
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

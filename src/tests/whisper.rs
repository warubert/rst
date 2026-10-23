use super::{
    command_args, is_unsupported_cuda_error, output_path, parse_progress_percent,
    parse_supported_languages,
};
use std::path::Path;

#[test]
fn builds_transcription_arguments_and_places_srt_next_to_video() {
    assert_eq!(
        command_args("media/episode one.mkv", "pt", Path::new("media"), None),
        [
            "--language",
            "pt",
            "--task",
            "transcribe",
            "--output_format",
            "srt",
            "--verbose",
            "False",
            "--output_dir",
            "media",
            "media/episode one.mkv"
        ]
    );
    assert_eq!(output_path("episode.mkv").to_string_lossy(), "episode.srt");
}

#[test]
fn adds_cpu_device_for_fallback_runs() {
    assert_eq!(
        command_args("video.mp4", "pt", Path::new("."), Some("cpu")),
        [
            "--language",
            "pt",
            "--task",
            "transcribe",
            "--output_format",
            "srt",
            "--verbose",
            "False",
            "--output_dir",
            ".",
            "--device",
            "cpu",
            "video.mp4"
        ]
    );
}

#[test]
fn writes_transcription_to_the_selected_output_directory() {
    assert_eq!(
        command_args("video_olbers.mp4", "pt", Path::new("custom-output"), None)[8..10],
        ["--output_dir", "custom-output"]
    );
}

#[test]
fn detects_cuda_kernel_compatibility_errors_for_cpu_fallback() {
    assert!(is_unsupported_cuda_error(
        "CUDA error: no kernel image is available for execution on the device"
    ));
    assert!(!is_unsupported_cuda_error("audio file does not exist"));
}

#[test]
fn parses_progress_from_tqdm_output() {
    assert_eq!(parse_progress_percent("  42%|████▏     | 21/50"), Some(42));
    assert_eq!(parse_progress_percent("100%|██████████| 50/50"), Some(100));
    assert_eq!(parse_progress_percent("loading model"), None);
    assert_eq!(parse_progress_percent("101%|bad"), None);
}

#[test]
fn limits_language_choices_to_codes_supported_by_the_cli() {
    let languages = parse_supported_languages(
        "usage: whisper [options]\n --language {en,pt,es,unknown} supported",
    )
    .unwrap();
    let codes: Vec<_> = languages.iter().map(|language| language.code()).collect();
    assert_eq!(codes, ["en", "es", "pt"]);
}

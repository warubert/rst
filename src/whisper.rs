use crate::{
    interfaces::whisper::{SubtitleTranscriber, TranscriptionEvent},
    languages::SupportedLanguage,
};
use rust_i18n::t;
use std::{
    collections::HashSet,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

fn supported_languages() -> Result<Vec<SupportedLanguage>, String> {
    let output = Command::new("whisper")
        .arg("--help")
        .output()
        .map_err(|error| t!("errors.whisper_unavailable", error = error).to_string())?;

    if !output.status.success() {
        return Err(t!(
            "errors.whisper_help",
            status = output.status,
            stderr = String::from_utf8_lossy(&output.stderr)
        )
        .to_string());
    }

    let help = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_supported_languages(&help)
}

fn parse_supported_languages(help: &str) -> Result<Vec<SupportedLanguage>, String> {
    let choices = help
        .split_once("--language {")
        .and_then(|(_, tail)| tail.split_once('}').map(|(choices, _)| choices))
        .ok_or_else(|| t!("errors.whisper_languages").to_string())?;
    let codes: HashSet<&str> = choices.split(',').map(str::trim).collect();
    let languages: Vec<_> = SupportedLanguage::ALL
        .iter()
        .copied()
        .filter(|language| codes.contains(language.code()))
        .collect();

    if languages.is_empty() {
        return Err(t!("errors.whisper_languages").to_string());
    }

    Ok(languages)
}

pub fn output_path(input: &str) -> PathBuf {
    Path::new(input).with_extension("srt")
}

fn output_parent(output: &Path) -> &Path {
    output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
}

fn command_args(
    input: &str,
    language: &str,
    output_directory: &Path,
    device: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "--language".to_string(),
        language.to_string(),
        "--task".to_string(),
        "transcribe".to_string(),
        "--output_format".to_string(),
        "srt".to_string(),
        "--verbose".to_string(),
        "False".to_string(),
        "--output_dir".to_string(),
        output_directory.to_string_lossy().into_owned(),
    ];
    if let Some(device) = device {
        args.extend(["--device".to_string(), device.to_string()]);
    }
    args.push(input.to_string());
    args
}

fn parse_progress_percent(line: &str) -> Option<u8> {
    let progress = line.split_once('%')?.0;
    let digits: String = progress
        .chars()
        .rev()
        .take_while(|character| character.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    (!digits.is_empty())
        .then(|| digits.parse::<u8>().ok())
        .flatten()
        .filter(|percent| *percent <= 100)
}

pub struct Whisper;

impl SubtitleTranscriber for Whisper {
    fn supported_languages(&self) -> Result<Vec<SupportedLanguage>, String> {
        supported_languages()
    }

    fn transcribe(
        &self,
        input: &str,
        language: &str,
        output: &str,
        on_event: &mut dyn FnMut(TranscriptionEvent),
    ) -> Result<PathBuf, String> {
        transcribe(input, language, output, on_event)
    }
}

fn transcribe_on_device(
    input: &str,
    language: &str,
    output_directory: &Path,
    device: Option<&str>,
    mut on_event: impl FnMut(TranscriptionEvent),
) -> Result<PathBuf, String> {
    let mut child = Command::new("whisper")
        .args(command_args(input, language, output_directory, device))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| t!("errors.whisper_start", error = error).to_string())?;
    let stderr = child
        .stderr
        .take()
        .expect("stderr was configured as a pipe");
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let mut stderr = BufReader::new(stderr);
        let mut bytes = Vec::new();
        loop {
            bytes.clear();
            match stderr.read_until(b'\r', &mut bytes) {
                Ok(0) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&bytes).trim().to_string();
                    if sender.send(line).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(error.to_string());
                    break;
                }
            }
        }
    });

    let mut latest_percent = 0;
    let mut stderr_lines = Vec::new();
    let status = loop {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                if let Some(percent) = parse_progress_percent(&line)
                    && percent > latest_percent
                {
                    latest_percent = percent;
                    on_event(TranscriptionEvent::Progress(percent));
                }
                if !line.is_empty() {
                    stderr_lines.push(line);
                    if stderr_lines.len() > 20 {
                        stderr_lines.remove(0);
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }

        if let Some(status) = child
            .try_wait()
            .map_err(|error| t!("errors.whisper_wait", error = error).to_string())?
        {
            break status;
        }
    };

    let _ = reader.join();
    while let Ok(line) = receiver.try_recv() {
        if let Some(percent) = parse_progress_percent(&line)
            && percent > latest_percent
        {
            latest_percent = percent;
            on_event(TranscriptionEvent::Progress(percent));
        }
        if !line.is_empty() {
            stderr_lines.push(line);
            if stderr_lines.len() > 20 {
                stderr_lines.remove(0);
            }
        }
    }

    if !status.success() {
        return Err(t!(
            "errors.whisper_status",
            status = status,
            stderr = stderr_lines.join("\n")
        )
        .to_string());
    }

    let input_output = output_path(input);
    let generated_name = input_output
        .file_name()
        .unwrap_or_else(|| Path::new("transcription.srt").as_os_str());
    Ok(output_directory.join(generated_name))
}

fn is_unsupported_cuda_error(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("no kernel image is available for execution on the device")
        || error.contains("not compatible with the current pytorch installation")
        || error.contains("does not include kernels for this gpu")
}

fn transcribe(
    input: &str,
    language: &str,
    output: &str,
    on_event: &mut dyn FnMut(TranscriptionEvent),
) -> Result<PathBuf, String> {
    let output = Path::new(output);
    let output_directory = create_staging_directory(output)?;
    let result =
        match transcribe_on_device(input, language, &output_directory, None, &mut *on_event) {
            Err(gpu_error) if is_unsupported_cuda_error(&gpu_error) => {
                on_event(TranscriptionEvent::CpuFallback);
                transcribe_on_device(
                    input,
                    language,
                    &output_directory,
                    Some("cpu"),
                    &mut *on_event,
                )
                .map_err(|cpu_error| {
                    t!(
                        "errors.whisper_cpu_fallback_failed",
                        gpu_error = gpu_error,
                        cpu_error = cpu_error
                    )
                    .to_string()
                })
            }
            result => result,
        }
        .and_then(|generated| publish_output(&generated, output));
    let _ = fs::remove_dir(&output_directory);
    result
}

fn create_staging_directory(output: &Path) -> Result<PathBuf, String> {
    let parent = output_parent(output);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..100 {
        let directory = parent.join(format!(
            ".rst-whisper-{}-{timestamp}-{attempt}",
            std::process::id()
        ));
        match fs::create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(t!(
                    "errors.whisper_output_directory",
                    path = directory.display(),
                    error = error
                )
                .to_string());
            }
        }
    }

    Err(t!("errors.whisper_output_directory_exists").to_string())
}

fn publish_output(generated: &Path, output: &Path) -> Result<PathBuf, String> {
    fs::rename(generated, output).map_err(|error| {
        t!(
            "errors.whisper_publish",
            generated = generated.display(),
            output = output.display(),
            error = error
        )
        .to_string()
    })?;
    Ok(output.to_path_buf())
}

#[cfg(test)]
#[path = "tests/whisper.rs"]
mod tests;

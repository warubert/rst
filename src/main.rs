use std::env;
use std::process::Command;
use serde_json::Value;

#[derive(Debug)]
struct Subtitle {
    index: i64,
    language: String,
    is_sdh: bool,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Please provide a command-line argument.");
        return;
    }

    let input = args[1].clone();

    let output = Command::new("echo")
        .arg("Comando echo executado pelo Rust")
        .output()
        .expect("Falha ao executar comando");

    if !output.status.success() {
        eprintln!("Comando terminou com erro: {}", output.status);
        return;
    }

    let ffprobe_output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
            &input,
        ])
        .output()
        .expect("Falha ao executar ffprobe");

    if !ffprobe_output.status.success() {
        let ffprobe_stderr = String::from_utf8_lossy(&ffprobe_output.stderr);
        eprintln!("Comando ffprobe terminou com erro: {}", ffprobe_output.status);
        eprintln!("stderr do ffprobe:\n{}", ffprobe_stderr);
        return;
    }

    let ffprobe_stdout = String::from_utf8_lossy(&ffprobe_output.stdout);
    // println!("Saida do ffprobe:\n{}", ffprobe_stdout);

    let ffprobe_json: Value = serde_json::from_str(&ffprobe_stdout)
        .expect("Falha ao converter a saida do ffprobe em JSON");

    let subtitle_streams: Vec<Subtitle> = ffprobe_json
        .get("streams")
        .and_then(Value::as_array)
        .map(|streams| {
            streams
                .iter()
                .filter(|stream| {
                    stream
                        .get("codec_type")
                        .and_then(Value::as_str)
                        == Some("subtitle")
                })
                .map(|stream| {
                    let index = stream
                        .get("index")
                        .and_then(Value::as_i64)
                        .unwrap_or(-1);
                    let language = stream
                        .get("tags")
                        .and_then(|tags| tags.get("language"))
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let is_sdh = stream
                        .get("disposition")
                        .and_then(|disposition| disposition.get("hearing_impaired"))
                        .and_then(Value::as_i64)
                        .map(|value| value == 1)
                        .unwrap_or(false);

                    Subtitle {
                        index,
                        language,
                        is_sdh,
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    for subtitle in &subtitle_streams {
        println!(
            "index: {}, language: {}, is_sdh: {}",
            subtitle.index, subtitle.language, subtitle.is_sdh
        );
    }

    let ffprobe_stderr = String::from_utf8_lossy(&ffprobe_output.stderr);
    if !ffprobe_stderr.trim().is_empty() {
        eprintln!("stderr do ffprobe:\n{}", ffprobe_stderr);
    }
}

use std::env;
use std::process::Command;

use serde_json::Value;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Please provide a command-line argument.");
        return;
    }

    let input = args[1].clone();
    let reversed = input.chars().rev().collect::<String>();
    println!("You typed: {}", input);
    println!("Reversed: {}", reversed);

    let output = Command::new("echo")
        .arg("Comando echo executado pelo Rust")
        .output()
        .expect("Falha ao executar comando");

    if !output.status.success() {
        eprintln!("Comando terminou com erro: {}", output.status);
        return;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    print!("Saida do comando: {}", stdout);

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
    println!("Saida do ffprobe:\n{}", ffprobe_stdout);

    let ffprobe_json: Value = serde_json::from_str(&ffprobe_stdout)
        .expect("Falha ao converter a saida do ffprobe em JSON");

    let subtitle_streams: Vec<Value> = ffprobe_json
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
                .cloned()
                .collect()
        })
        .unwrap_or_default();

    println!("Legendas encontradas: {}", subtitle_streams.len());
    println!("subtitle_streams: {:#?}", subtitle_streams);

    let ffprobe_stderr = String::from_utf8_lossy(&ffprobe_output.stderr);
    if !ffprobe_stderr.trim().is_empty() {
        eprintln!("stderr do ffprobe:\n{}", ffprobe_stderr);
    }
}

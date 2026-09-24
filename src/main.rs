use std::env;
use std::io::{self, Write};
use std::path::Path;
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

    if subtitle_streams.is_empty() {
        println!("Nenhuma legenda encontrada no arquivo.");
        return;
    }

    println!("\nOpcoes de linguas encontradas:");
    for (position, subtitle) in subtitle_streams.iter().enumerate() {
        let sdh_label = if subtitle.is_sdh { " (SDH)" } else { "" };
        println!("- {}: {}{}", position + 1, subtitle.language, sdh_label);
    }

    let selected_subtitle = loop {
        print!("\nDigite o numero da opcao desejada: ");
        io::stdout().flush().expect("Falha ao atualizar terminal");

        let mut input_line = String::new();
        io::stdin()
            .read_line(&mut input_line)
            .expect("Falha ao ler entrada do usuario");

        let parsed_option = match input_line.trim().parse::<usize>() {
            Ok(value) => value,
            Err(_) => {
                println!("Entrada invalida. Digite um numero inteiro.");
                continue;
            }
        };

        if parsed_option == 0 || parsed_option > subtitle_streams.len() {
            println!("Opcao invalida. Escolha um numero da lista.");
            continue;
        }

        break &subtitle_streams[parsed_option - 1];
    };

    println!(
        "Legenda selecionada -> index: {}, language: {}, is_sdh: {}",
        selected_subtitle.index, selected_subtitle.language, selected_subtitle.is_sdh
    );

    let default_output_name = Path::new(&input)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("legenda")
        .to_string();

    let mut custom_name = String::new();
    print!(
        "\nDigite o nome da legenda de saida (padrao: {}.srt): ",
        default_output_name
    );
    io::stdout().flush().expect("Falha ao atualizar terminal");

    io::stdin()
        .read_line(&mut custom_name)
        .expect("Falha ao ler o nome da legenda");

    let final_output_name = match custom_name.trim() {
        "" => format!("{}.srt", default_output_name),
        value => {
            if value.to_lowercase().ends_with(".srt") {
                value.to_string()
            } else {
                format!("{}.srt", value)
            }
        }
    };

    let output_srt_string = final_output_name;
    let map_argument = format!("0:{}", selected_subtitle.index);

    println!(
        "Executando: ffmpeg -i {} -map {} {}",
        input, map_argument, output_srt_string
    );

    let ffmpeg_output = Command::new("ffmpeg")
        .args(["-i", &input, "-map", &map_argument, &output_srt_string])
        .output()
        .expect("Falha ao executar ffmpeg");

    if !ffmpeg_output.status.success() {
        let ffmpeg_stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
        eprintln!("Comando ffmpeg terminou com erro: {}", ffmpeg_output.status);
        eprintln!("stderr do ffmpeg:\n{}", ffmpeg_stderr);
        return;
    }

    println!("Legenda extraida com sucesso em: {}", output_srt_string);

    let ffprobe_stderr = String::from_utf8_lossy(&ffprobe_output.stderr);
    if !ffprobe_stderr.trim().is_empty() {
        eprintln!("stderr do ffprobe:\n{}", ffprobe_stderr);
    }
}

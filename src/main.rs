use std::env;
use std::io::{self, Write};
use std::path::Path;

mod ffmpeg;
mod ffprobe;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("Please provide a command-line argument.");
        return;
    }

    let input = args[1].clone();

    let subtitle_streams = match ffprobe::get_subtitle_streams(&input) {
        Ok(streams) => streams,
        Err(error) => {
            eprintln!("{}", error);
            return;
        }
    };

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
        print!("\nDigite o numero da opcao desejada (padrao: 1): ");
        io::stdout().flush().expect("Falha ao atualizar terminal");

        let mut input_line = String::new();
        io::stdin()
            .read_line(&mut input_line)
            .expect("Falha ao ler entrada do usuario");

        let trimmed = input_line.trim();
        let parsed_option = if trimmed.is_empty() {
            1
        } else {
            match trimmed.parse::<usize>() {
                Ok(value) => value,
                Err(_) => {
                    println!("Entrada invalida. Digite um numero inteiro.");
                    continue;
                }
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

    if let Err(error) = ffmpeg::extract_subtitle(&input, selected_subtitle.index, &output_srt_string) {
        eprintln!("{}", error);
        return;
    }

    println!("Legenda extraida com sucesso em: {}", output_srt_string);
}

use crate::ffprobe::Subtitle;
use std::io::{self, Write};
use std::path::Path;

pub fn print_error(message: &str) {
    eprintln!("{}", message);
}

pub fn print_no_subtitles_found() {
    println!("Nenhuma legenda encontrada no arquivo.");
}

pub fn select_subtitle(subtitle_streams: &[Subtitle]) -> &Subtitle {
    println!("\nOpcoes de linguas encontradas:");
    for (position, subtitle) in subtitle_streams.iter().enumerate() {
        let sdh_label = if subtitle.is_sdh { " (SDH)" } else { "" };
        println!("- {}: {}{}", position + 1, subtitle.language, sdh_label);
    }

    loop {
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

        return &subtitle_streams[parsed_option - 1];
    }
}

pub fn prompt_output_name(input: &str) -> String {
    let default_output_name = Path::new(input)
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

    match custom_name.trim() {
        "" => format!("{}.srt", default_output_name),
        value => {
            if value.to_lowercase().ends_with(".srt") {
                value.to_string()
            } else {
                format!("{}.srt", value)
            }
        }
    }
}

pub fn print_ffmpeg_command(input: &str, subtitle_index: i64, output_srt: &str) {
    println!(
        "Executando: ffmpeg -i {} -map 0:{} {}",
        input, subtitle_index, output_srt
    );
}

pub fn print_success(output_srt: &str) {
    println!("Legenda extraida com sucesso em: {}", output_srt);
}

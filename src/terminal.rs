pub fn print_error(message: &str) {
    eprintln!("{}", message);
}

pub fn print_no_subtitles_found() {
    println!("Nenhuma legenda encontrada no arquivo.");
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

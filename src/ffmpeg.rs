use std::process::Command;

pub fn extract_subtitle(input: &str, subtitle_index: i64, output_srt: &str) -> Result<(), String> {
    let map_argument = format!("0:{subtitle_index}");

    println!(
        "Executando: ffmpeg -i {} -map {} {}",
        input, map_argument, output_srt
    );

    let ffmpeg_output = Command::new("ffmpeg")
        .args(["-i", input, "-map", &map_argument, output_srt])
        .output()
        .map_err(|error| format!("Falha ao executar ffmpeg: {error}"))?;

    if !ffmpeg_output.status.success() {
        let ffmpeg_stderr = String::from_utf8_lossy(&ffmpeg_output.stderr);
        return Err(format!(
            "Comando ffmpeg terminou com erro: {}\nstderr do ffmpeg:\n{}",
            ffmpeg_output.status, ffmpeg_stderr
        ));
    }

    Ok(())
}

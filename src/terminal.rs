use rust_i18n::t;

pub fn print_error(message: &str) {
    eprintln!("{}", message);
}

pub fn print_no_subtitles_found() {
    println!("{}", t!("terminal.no_subtitles"));
}

pub fn print_ffmpeg_command(input: &str, subtitle_index: i64, output_srt: &str) {
    println!(
        "{}",
        t!(
            "terminal.command",
            input = input,
            index = subtitle_index,
            output = output_srt
        )
    );
}

pub fn print_success(output_srt: &str) {
    println!("{}", t!("terminal.success", output = output_srt));
}

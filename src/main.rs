rust_i18n::i18n!("locales", fallback = "en");

use std::env;

mod cli;
mod ffmpeg;
mod ffprobe;
mod interactive;
mod interfaces;
mod language_names;
mod languages;
mod localization;
mod messages;
mod settings;
mod srt;
mod translate;
mod tui;
mod whisper;
mod workflow;

fn main() {
    let _ = dotenvy::dotenv();
    localization::initialize();

    let arguments: Vec<String> = env::args().skip(1).collect();
    if arguments.is_empty() {
        interactive::run();
        return;
    }

    match cli::parse_args(&arguments) {
        Ok(cli::CliParse::Help) => println!("{}", cli::USAGE),
        Ok(cli::CliParse::Command(command)) => {
            if let Err(error) = cli::run(command) {
                eprintln!("Error: {error}");
                std::process::exit(1);
            }
        }
        Err(error) => {
            eprintln!("Error: {error}\n\n{}", cli::USAGE);
            std::process::exit(2);
        }
    }
}

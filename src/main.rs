use std::env;
mod ffmpeg;
mod ffprobe;
mod terminal;
mod tui;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        terminal::print_error("Please provide a command-line argument.");
        return;
    }

    let input = args[1].clone();

    let subtitle_streams = match ffprobe::get_subtitle_streams(&input) {
        Ok(streams) => streams,
        Err(error) => {
            terminal::print_error(&error);
            return;
        }
    };

    if subtitle_streams.is_empty() {
        terminal::print_no_subtitles_found();
        return;
    }

    let selected_index = tui::select_subtitle(&subtitle_streams);
    let selected_subtitle = &subtitle_streams[selected_index];
    let output_srt_string = tui::prompt_output_name(&input);

    terminal::print_ffmpeg_command(&input, selected_subtitle.index, &output_srt_string);

    if let Err(error) = ffmpeg::extract_subtitle(&input, selected_subtitle.index, &output_srt_string) {
        terminal::print_error(&error);
        return;
    }

    terminal::print_success(&output_srt_string);
}

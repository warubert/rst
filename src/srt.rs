use rust_i18n::t;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    pub index: String,
    pub timing: String,
    pub text: String,
}

pub fn parse(content: &str) -> Result<Vec<Cue>, String> {
    let content = content.trim_start_matches('\u{feff}');
    let mut cues = Vec::new();
    let mut block: Vec<&str> = Vec::new();

    for line in content.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if !block.is_empty() {
                cues.push(parse_block(&block, cues.len() + 1)?);
                block.clear();
            }
        } else {
            block.push(line);
        }
    }

    if cues.is_empty() {
        return Err(t!("errors.srt_empty").to_string());
    }

    Ok(cues)
}

fn parse_block(lines: &[&str], position: usize) -> Result<Cue, String> {
    match lines {
        [index, timing, text @ ..] if timing.contains("-->") => Ok(Cue {
            index: index.trim().to_string(),
            timing: timing.trim().to_string(),
            text: text.join("\n"),
        }),
        _ => Err(t!("errors.srt_invalid", block = position).to_string()),
    }
}

pub fn serialize(cues: &[Cue]) -> String {
    cues.iter()
        .map(|cue| format!("{}\n{}\n{}\n", cue.index, cue.timing, cue.text))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Splits positioning tags such as `{\an8}` from the start of a cue text.
pub fn split_leading_tags(text: &str) -> (&str, &str) {
    let mut end = 0;
    while text[end..].starts_with("{\\") {
        match text[end..].find('}') {
            Some(close) => end += close + 1,
            None => break,
        }
    }
    text.split_at(end)
}

/// Breaks a long single-line text at the space closest to its middle.
pub fn balance_lines(text: &str, max_line_chars: usize) -> String {
    let total = text.chars().count();
    if total <= max_line_chars || text.contains('\n') {
        return text.to_string();
    }

    let middle = total / 2;
    let split_at = text
        .char_indices()
        .enumerate()
        .filter(|(_, (_, character))| *character == ' ')
        .min_by_key(|(position, _)| position.abs_diff(middle))
        .map(|(_, (byte_index, _))| byte_index);

    match split_at {
        Some(byte_index) => format!("{}\n{}", &text[..byte_index], &text[byte_index + 1..]),
        None => text.to_string(),
    }
}

#[cfg(test)]
#[path = "tests/srt.rs"]
mod tests;

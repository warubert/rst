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
mod tests {
    use super::{Cue, balance_lines, parse, serialize, split_leading_tags};

    const SAMPLE: &str = "1\n00:00:07,080 --> 00:00:09,820\n{\\an8}This is what I'm eating\nfrom this moose hide.\n\n2\n00:00:09,920 --> 00:00:13,060\n{\\an8}<i>I like this experience,\nbut I wouldn't like it</i>\n";

    #[test]
    fn parses_multiline_cues_with_tags() {
        let cues = parse(SAMPLE).unwrap();

        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].index, "1");
        assert_eq!(cues[0].timing, "00:00:07,080 --> 00:00:09,820");
        assert_eq!(
            cues[0].text,
            "{\\an8}This is what I'm eating\nfrom this moose hide."
        );
        assert!(cues[1].text.starts_with("{\\an8}<i>I like"));
    }

    #[test]
    fn accepts_crlf_and_byte_order_mark() {
        let content = format!("\u{feff}{}", SAMPLE.replace('\n', "\r\n"));
        let cues = parse(&content).unwrap();

        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].index, "1");
        assert!(!cues[0].text.contains('\r'));
    }

    #[test]
    fn serialize_round_trips_parsed_content() {
        let cues = parse(SAMPLE).unwrap();
        assert_eq!(serialize(&cues), SAMPLE);
    }

    #[test]
    fn rejects_empty_and_malformed_files() {
        assert!(parse("").is_err());
        assert!(parse("\n\n").is_err());
        assert!(parse("1\nnot a timing\ntext").is_err());
    }

    #[test]
    fn splits_positioning_tags_from_text() {
        assert_eq!(split_leading_tags("{\\an8}Hello"), ("{\\an8}", "Hello"));
        assert_eq!(
            split_leading_tags("{\\an8}{\\i1}Hello"),
            ("{\\an8}{\\i1}", "Hello")
        );
        assert_eq!(split_leading_tags("Hello {\\an8}"), ("", "Hello {\\an8}"));
        assert_eq!(split_leading_tags("{\\an8"), ("", "{\\an8"));
        assert_eq!(split_leading_tags("{\\an8}"), ("{\\an8}", ""));
    }

    #[test]
    fn balances_only_long_single_line_texts() {
        assert_eq!(balance_lines("short text", 42), "short text");
        assert_eq!(
            balance_lines("already\nbroken text that is long", 5),
            "already\nbroken text that is long"
        );
        assert_eq!(
            balance_lines("um dois tres quatro cinco seis sete oito nove dez", 20),
            "um dois tres quatro cinco\nseis sete oito nove dez"
        );
        assert_eq!(
            balance_lines("unbrokenwordthatisverylong", 5),
            "unbrokenwordthatisverylong"
        );
    }

    #[test]
    fn cue_equality_is_structural() {
        let cue = Cue {
            index: "1".into(),
            timing: "t --> t".into(),
            text: "x".into(),
        };
        assert_eq!(cue.clone(), cue);
    }
}

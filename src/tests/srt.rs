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

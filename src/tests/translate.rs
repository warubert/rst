use super::cues::{
    MAX_ATTEMPTS, assemble, chunk_ranges, plan_cues, translate_cues, translate_cues_with_progress,
};
use super::providers::{build_request_body, parse_response};
use super::source_language_code;
use super::text::{escape_html, unescape_html};
use crate::interfaces::translate::Translator;
use crate::srt::Cue;
use rust_i18n::t;
use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

fn cue(index: &str, text: &str) -> Cue {
    Cue {
        index: index.to_string(),
        timing: "00:00:01,000 --> 00:00:02,000".to_string(),
        text: text.to_string(),
    }
}

struct Uppercase {
    batch_sizes: RefCell<Vec<usize>>,
    languages: RefCell<Vec<(String, String)>>,
}

impl Uppercase {
    fn new() -> Self {
        Self {
            batch_sizes: RefCell::new(Vec::new()),
            languages: RefCell::new(Vec::new()),
        }
    }
}

impl Translator for Uppercase {
    async fn translate_batch(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
    ) -> Result<Vec<String>, String> {
        self.batch_sizes.borrow_mut().push(texts.len());
        self.languages
            .borrow_mut()
            .push((source.to_string(), target.to_string()));
        Ok(texts.iter().map(|text| text.to_uppercase()).collect())
    }
}

struct Flaky {
    failures_left: Cell<u32>,
    calls: Cell<u32>,
}

impl Translator for Flaky {
    async fn translate_batch(
        &self,
        texts: &[String],
        _source: &str,
        _target: &str,
    ) -> Result<Vec<String>, String> {
        self.calls.set(self.calls.get() + 1);
        if self.failures_left.get() > 0 {
            self.failures_left.set(self.failures_left.get() - 1);
            return Err("temporary failure".to_string());
        }
        Ok(texts.to_vec())
    }
}

#[test]
fn builds_request_body_in_the_expected_shape() {
    let texts = vec!["Hello".to_string(), "It's \"fine\"".to_string()];
    assert_eq!(
        build_request_body(&texts, "auto", "pt"),
        r#"[[["Hello","It's \"fine\""],"auto","pt"],"te"]"#
    );
}

#[test]
fn parses_response_and_decodes_entities() {
    let body =
        r#"[["Eu n&#39;ao sei","<i>Eca!</i>","Tom &amp; Jerry &quot;ok&quot;"],["en","en","en"]]"#;
    assert_eq!(
        parse_response(body, 3).unwrap(),
        ["Eu n'ao sei", "<i>Eca!</i>", "Tom & Jerry \"ok\""]
    );
}

#[test]
fn parses_response_without_detected_languages() {
    assert_eq!(
        parse_response(r#"[["Hola Mundo."]]"#, 1).unwrap(),
        ["Hola Mundo."]
    );
}

#[test]
fn rejects_malformed_or_mismatched_responses() {
    assert!(parse_response("not json", 1).is_err());
    assert!(parse_response(r#"{"a":1}"#, 1).is_err());
    assert!(parse_response(r#"[[1,2]]"#, 2).is_err());
    assert!(parse_response(r#"[["only one"]]"#, 2).is_err());
}

#[test]
fn escapes_ampersands_and_decodes_numeric_entities() {
    assert_eq!(escape_html("R&B <i>x</i>"), "R&amp;B <i>x</i>");
    assert_eq!(unescape_html("&#39;a&#x27;b&lt;&gt;&amp;"), "'a'b<>&");
    assert_eq!(
        unescape_html("fish & chips; &unknown; &#zz;"),
        "fish & chips; &unknown; &#zz;"
    );
    assert_eq!(unescape_html("no entities"), "no entities");
}

#[test]
fn chunks_by_item_count_and_character_budget() {
    let texts: Vec<String> = (0..5).map(|_| "abcd".to_string()).collect();

    assert_eq!(chunk_ranges(&texts, 2, 1000), [0..2, 2..4, 4..5]);
    assert_eq!(chunk_ranges(&texts, 100, 9), [0..2, 2..4, 4..5]);
    assert_eq!(
        chunk_ranges(&[], 2, 10),
        Vec::<std::ops::Range<usize>>::new()
    );
    assert_eq!(chunk_ranges(&["x".repeat(50)], 2, 10), vec![0..1]);
}

#[test]
fn joins_wrapped_lines_but_keeps_dialogue_separate() {
    let cues = [
        cue("1", "{\\an8}This is what I'm eating\nfrom this moose hide."),
        cue("2", "- Hello.\n- Hi there."),
        cue("3", "{\\an8}"),
    ];

    let (plans, texts) = plan_cues(&cues);

    assert_eq!(
        texts,
        [
            "This is what I'm eating from this moose hide.",
            "- Hello.",
            "- Hi there."
        ]
    );
    assert_eq!(plans[0].prefix, "{\\an8}");
    assert!(!plans[0].dialogue);
    assert!(plans[1].dialogue);
    assert_eq!(plans[2].segments, 0);
}

#[test]
fn assembles_translated_text_with_tags_and_balanced_lines() {
    let source = cue("1", "{\\an8}short");
    let (plans, _) = plan_cues(std::slice::from_ref(&source));

    let single = assemble(
        &source,
        &plans[0],
        &["um dois tres quatro cinco seis sete oito nove dez".to_string()],
    );

    assert_eq!(
        single.text,
        "{\\an8}um dois tres quatro cinco\nseis sete oito nove dez"
    );
    assert_eq!(single.index, "1");
    assert_eq!(single.timing, source.timing);
}

#[tokio::test]
async fn translates_cues_preserving_order_timing_and_tags() {
    let translator = Uppercase::new();
    let cues = [
        cue("1", "{\\an8}hello\nworld"),
        cue("2", "<i>bye</i>"),
        cue("3", "- yes\n- no"),
        cue("4", ""),
    ];

    let translated = translate_cues(&translator, &cues, "en", "pt", Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(translated.len(), 4);
    assert_eq!(translated[0].text, "{\\an8}HELLO WORLD");
    assert_eq!(translated[1].text, "<I>BYE</I>");
    assert_eq!(translated[2].text, "- YES\n- NO");
    assert_eq!(translated[3].text, "");
    assert_eq!(translated[2].index, "3");
    assert_eq!(translated[1].timing, cues[1].timing);
    assert_eq!(
        translator.languages.borrow()[0],
        ("en".to_string(), "pt".to_string())
    );
}

#[tokio::test]
async fn reports_progress_as_translation_batches_complete() {
    let translator = Uppercase::new();
    let cues = (0..101)
        .map(|index| cue(&index.to_string(), "hello"))
        .collect::<Vec<_>>();
    let mut progress = Vec::new();

    let translated = translate_cues_with_progress(
        &translator,
        &cues,
        "en",
        "pt",
        Duration::ZERO,
        &mut |percent| progress.push(percent),
    )
    .await
    .unwrap();

    assert_eq!(translated.len(), 101);
    assert_eq!(progress, [50, 100]);
}

#[tokio::test]
async fn splits_large_subtitles_into_multiple_requests() {
    let translator = Uppercase::new();
    let cues: Vec<Cue> = (1..=250)
        .map(|number| cue(&number.to_string(), &format!("line {number}")))
        .collect();

    let translated = translate_cues(&translator, &cues, "auto", "es", Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(*translator.batch_sizes.borrow(), [100, 100, 50]);
    assert_eq!(translated[0].text, "LINE 1");
    assert_eq!(translated[249].text, "LINE 250");
}

#[tokio::test]
async fn retries_transient_failures_and_then_succeeds() {
    let translator = Flaky {
        failures_left: Cell::new(MAX_ATTEMPTS - 1),
        calls: Cell::new(0),
    };

    let translated = translate_cues(&translator, &[cue("1", "hi")], "en", "pt", Duration::ZERO)
        .await
        .unwrap();

    assert_eq!(translated[0].text, "hi");
    assert_eq!(translator.calls.get(), MAX_ATTEMPTS);
}

#[test]
fn maps_subtitle_language_tags_to_translation_source_codes() {
    assert_eq!(source_language_code("en"), "en");
    assert_eq!(source_language_code("eng"), "en");
    assert_eq!(source_language_code("pt_BR"), "pt-br");
    assert_eq!(source_language_code("por"), "pt");
    assert_eq!(source_language_code("und"), "auto");
    assert_eq!(source_language_code(""), "auto");
    assert_eq!(
        source_language_code(&t!("tui.unknown_language", locale = "en")),
        "auto"
    );
}

#[test]
fn parses_gtx_segments_into_one_line_per_input() {
    let body = r#"[[["É realmente interessante\n","It's really interesting\n",null,null,10],["para ver quem você se torna\n","to see who you become\n"],["Eca!","Ugh!"]],null,"en"]"#;

    assert_eq!(
        super::providers::parse_gtx_response(body, 3).unwrap(),
        [
            "É realmente interessante",
            "para ver quem você se torna",
            "Eca!"
        ]
    );
}

#[test]
fn rejects_gtx_responses_with_a_different_line_count() {
    let body = r#"[[["um\ndois\n","x"]],null,"en"]"#;

    assert!(super::providers::parse_gtx_response(body, 3).is_err());
    assert!(super::providers::parse_gtx_response("not json", 1).is_err());
    assert!(super::providers::parse_gtx_response(r#"{"a":1}"#, 1).is_err());
}

// Requires network access and the key in the environment or .env.
#[test]
#[ignore]
fn compares_keyless_gtx_with_translate_html_on_the_sample_file() {
    let _ = dotenvy::dotenv();
    let input = concat!(env!("CARGO_MANIFEST_DIR"), "/AloneS13E13.srt");
    let cues = crate::srt::parse(&std::fs::read_to_string(input).unwrap()).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let google = super::GoogleTranslator::new(super::api_key().unwrap());
    let gtx = super::GtxTranslator::new();
    let expected = runtime
        .block_on(translate_cues(
            &google,
            &cues,
            "auto",
            "pt",
            Duration::from_millis(500),
        ))
        .unwrap();
    let started = std::time::Instant::now();
    let actual = runtime
        .block_on(translate_cues(
            &gtx,
            &cues,
            "auto",
            "pt",
            Duration::from_millis(500),
        ))
        .unwrap();
    println!("gtx took {:?}", started.elapsed());

    assert_eq!(expected.len(), actual.len());
    let mut identical = 0;
    let mut differences = Vec::new();
    for (left, right) in expected.iter().zip(&actual) {
        assert_eq!(left.index, right.index);
        assert_eq!(left.timing, right.timing);
        if left.text == right.text {
            identical += 1;
        } else {
            differences.push((left.index.clone(), left.text.clone(), right.text.clone()));
        }
    }

    println!("identical: {identical}/{}", expected.len());
    for (index, left, right) in differences.iter().take(25) {
        println!("--- cue {index}\n translateHtml: {left:?}\n gtx:           {right:?}");
    }
}

#[tokio::test]
async fn gives_up_after_the_maximum_number_of_attempts() {
    let translator = Flaky {
        failures_left: Cell::new(MAX_ATTEMPTS),
        calls: Cell::new(0),
    };

    let result = translate_cues(&translator, &[cue("1", "hi")], "en", "pt", Duration::ZERO).await;

    assert_eq!(result.unwrap_err(), "temporary failure");
    assert_eq!(translator.calls.get(), MAX_ATTEMPTS);
}

// Requires network access and the key in the environment or .env:
// cargo test translates_the_sample_file -- --ignored
#[test]
#[ignore]
fn translates_the_sample_file_with_the_real_service() {
    let _ = dotenvy::dotenv();
    let api_key = super::api_key().expect("set GOOGLE_TRANSLATE_API_KEY");

    assert_sample_translation(Some(&api_key));
}

// Requires network access only.
#[test]
#[ignore]
fn translates_the_sample_file_without_an_api_key() {
    assert_sample_translation(None);
}

fn assert_sample_translation(api_key: Option<&str>) {
    let input = concat!(env!("CARGO_MANIFEST_DIR"), "/AloneS13E13.srt");
    let output =
        std::env::temp_dir().join(format!("AloneS13E13-pt-test-{}.srt", api_key.is_some()));
    let output = output.to_str().unwrap();

    super::translate_file(input, output, "auto", "pt", api_key).unwrap();

    let original = crate::srt::parse(&std::fs::read_to_string(input).unwrap()).unwrap();
    let translated = crate::srt::parse(&std::fs::read_to_string(output).unwrap()).unwrap();
    std::fs::remove_file(output).unwrap();

    assert_eq!(original.len(), translated.len());
    for (before, after) in original.iter().zip(&translated) {
        assert_eq!(before.index, after.index);
        assert_eq!(before.timing, after.timing);
        assert_eq!(
            before.text.starts_with("{\\an8}"),
            after.text.starts_with("{\\an8}")
        );
        assert_eq!(before.text.is_empty(), after.text.is_empty());
    }
    assert_ne!(original[0].text, translated[0].text);
    println!("{}", translated[0].text);
}

use crate::interfaces::translate::Translator;
use crate::srt::{self, Cue};
use futures_util::{StreamExt, TryStreamExt, stream};
use rust_i18n::t;
use serde_json::{Value, json};
use std::{env, fs, ops::Range, time::Duration};

pub const API_KEY_ENV: &str = "GOOGLE_TRANSLATE_API_KEY";
const ENDPOINT: &str = "https://translate-pa.googleapis.com/v1/translateHtml";
const GTX_ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";
const MAX_BATCH_ITEMS: usize = 100;
const MAX_BATCH_CHARS: usize = 4000;
const MAX_CONCURRENT_REQUESTS: usize = 4;
const MAX_ATTEMPTS: u32 = 3;
const RETRY_DELAY: Duration = Duration::from_millis(500);
const MAX_LINE_CHARS: usize = 42;

pub struct GoogleTranslator {
    client: reqwest::Client,
    api_key: String,
}

impl GoogleTranslator {
    pub fn new(api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
        }
    }
}

pub fn api_key() -> Option<String> {
    read_api_key(env::var(API_KEY_ENV).ok())
}

fn read_api_key(value: Option<String>) -> Option<String> {
    value
        .map(|key| key.trim().to_string())
        .filter(|key| !key.is_empty())
}

impl Translator for GoogleTranslator {
    async fn translate_batch(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
    ) -> Result<Vec<String>, String> {
        let response = self
            .client
            .post(ENDPOINT)
            .header("content-type", "application/json+protobuf")
            .header("x-goog-api-key", &self.api_key)
            .body(build_request_body(texts, source, target))
            .send()
            .await
            .map_err(|error| t!("errors.translate_request", error = error).to_string())?;

        let status = response.status();
        if !status.is_success() {
            return Err(t!("errors.translate_status", status = status).to_string());
        }

        let body = response
            .text()
            .await
            .map_err(|error| t!("errors.translate_request", error = error).to_string())?;

        parse_response(&body, texts.len())
    }
}

pub struct GtxTranslator {
    client: reqwest::Client,
}

impl GtxTranslator {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Translator for GtxTranslator {
    async fn translate_batch(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
    ) -> Result<Vec<String>, String> {
        let joined = texts
            .iter()
            .map(|text| unescape_html(text))
            .collect::<Vec<_>>()
            .join("\n");

        let response = self
            .client
            .post(GTX_ENDPOINT)
            .query(&[
                ("client", "gtx"),
                ("sl", source),
                ("tl", target),
                ("dt", "t"),
            ])
            .form(&[("q", joined)])
            .send()
            .await
            .map_err(|error| t!("errors.translate_request", error = error).to_string())?;

        let status = response.status();
        if !status.is_success() {
            return Err(t!("errors.translate_status", status = status).to_string());
        }

        let body = response
            .text()
            .await
            .map_err(|error| t!("errors.translate_request", error = error).to_string())?;

        parse_gtx_response(&body, texts.len())
    }
}

fn parse_gtx_response(body: &str, expected: usize) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| t!("errors.translate_response", error = error).to_string())?;

    let segments = value
        .get(0)
        .and_then(Value::as_array)
        .ok_or_else(|| t!("errors.translate_response", error = "unexpected format").to_string())?;

    let mut joined = String::new();
    for segment in segments {
        match segment.get(0).and_then(Value::as_str) {
            Some(text) => joined.push_str(text),
            None => {
                return Err(
                    t!("errors.translate_response", error = "unexpected format").to_string()
                );
            }
        }
    }

    let joined = joined.strip_suffix('\n').unwrap_or(&joined);
    let lines: Vec<String> = joined.split('\n').map(str::to_string).collect();

    if lines.len() != expected {
        return Err(t!(
            "errors.translate_mismatch",
            expected = expected,
            received = lines.len()
        )
        .to_string());
    }

    Ok(lines)
}

pub fn translate_file(
    input: &str,
    output: &str,
    source: &str,
    target: &str,
    api_key: Option<&str>,
) -> Result<(), String> {
    let content = fs::read_to_string(input)
        .map_err(|error| t!("errors.srt_read", path = input, error = error).to_string())?;
    let cues = srt::parse(&content)?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| t!("errors.translate_request", error = error).to_string())?;
    let translated = match api_key {
        Some(key) => runtime.block_on(translate_cues(
            &GoogleTranslator::new(key.to_string()),
            &cues,
            source,
            target,
            RETRY_DELAY,
        )),
        None => runtime.block_on(translate_cues(
            &GtxTranslator::new(),
            &cues,
            source,
            target,
            RETRY_DELAY,
        )),
    }?;

    fs::write(output, srt::serialize(&translated))
        .map_err(|error| t!("errors.srt_write", path = output, error = error).to_string())
}

pub async fn translate_cues<T: Translator>(
    translator: &T,
    cues: &[Cue],
    source: &str,
    target: &str,
    retry_delay: Duration,
) -> Result<Vec<Cue>, String> {
    let (plans, texts) = plan_cues(cues);
    let ranges = chunk_ranges(&texts, MAX_BATCH_ITEMS, MAX_BATCH_CHARS);

    let batches: Vec<Vec<String>> = stream::iter(ranges)
        .map(|range| translate_with_retry(translator, &texts[range], source, target, retry_delay))
        .buffered(MAX_CONCURRENT_REQUESTS)
        .try_collect()
        .await?;
    let translated: Vec<String> = batches.into_iter().flatten().collect();

    let mut offset = 0;
    let mut result = Vec::with_capacity(cues.len());
    for (cue, plan) in cues.iter().zip(&plans) {
        let segments = &translated[offset..offset + plan.segments];
        offset += plan.segments;
        result.push(assemble(cue, plan, segments));
    }

    Ok(result)
}

async fn translate_with_retry<T: Translator>(
    translator: &T,
    texts: &[String],
    source: &str,
    target: &str,
    retry_delay: Duration,
) -> Result<Vec<String>, String> {
    let mut attempt = 1;
    loop {
        match translator.translate_batch(texts, source, target).await {
            Ok(translated) => return Ok(translated),
            Err(error) if attempt >= MAX_ATTEMPTS => return Err(error),
            Err(_) => {
                tokio::time::sleep(retry_delay * attempt).await;
                attempt += 1;
            }
        }
    }
}

struct CuePlan {
    prefix: String,
    dialogue: bool,
    segments: usize,
}

fn plan_cues(cues: &[Cue]) -> (Vec<CuePlan>, Vec<String>) {
    let mut plans = Vec::with_capacity(cues.len());
    let mut texts = Vec::new();

    for cue in cues {
        let (prefix, body) = srt::split_leading_tags(&cue.text);
        let lines: Vec<&str> = body
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let dialogue = lines.len() > 1 && lines.iter().any(|line| line.starts_with('-'));

        let segments: Vec<String> = if lines.is_empty() {
            Vec::new()
        } else if dialogue {
            lines.iter().map(|line| escape_html(line)).collect()
        } else {
            vec![escape_html(&lines.join(" "))]
        };

        plans.push(CuePlan {
            prefix: prefix.to_string(),
            dialogue,
            segments: segments.len(),
        });
        texts.extend(segments);
    }

    (plans, texts)
}

fn assemble(cue: &Cue, plan: &CuePlan, translated: &[String]) -> Cue {
    let text = match translated {
        [] => String::new(),
        _ if plan.dialogue => translated.join("\n"),
        [single] => srt::balance_lines(single, MAX_LINE_CHARS),
        _ => translated.join(" "),
    };

    Cue {
        text: format!("{}{}", plan.prefix, text),
        ..cue.clone()
    }
}

fn chunk_ranges(texts: &[String], max_items: usize, max_chars: usize) -> Vec<Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = 0;
    let mut chars = 0;

    for (index, text) in texts.iter().enumerate() {
        let length = text.chars().count();
        if index > start && (index - start >= max_items || chars + length > max_chars) {
            ranges.push(start..index);
            start = index;
            chars = 0;
        }
        chars += length;
    }

    if start < texts.len() {
        ranges.push(start..texts.len());
    }

    ranges
}

fn build_request_body(texts: &[String], source: &str, target: &str) -> String {
    json!([[texts, source, target], "te"]).to_string()
}

fn parse_response(body: &str, expected: usize) -> Result<Vec<String>, String> {
    let value: Value = serde_json::from_str(body)
        .map_err(|error| t!("errors.translate_response", error = error).to_string())?;

    let translations: Vec<String> = value
        .get(0)
        .and_then(Value::as_array)
        .ok_or_else(|| t!("errors.translate_response", error = "unexpected format").to_string())?
        .iter()
        .map(|item| item.as_str().map(unescape_html))
        .collect::<Option<_>>()
        .ok_or_else(|| t!("errors.translate_response", error = "unexpected format").to_string())?;

    if translations.len() != expected {
        return Err(t!(
            "errors.translate_mismatch",
            expected = expected,
            received = translations.len()
        )
        .to_string());
    }

    Ok(translations)
}

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
}

fn unescape_html(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(position) = rest.find('&') {
        result.push_str(&rest[..position]);
        rest = &rest[position..];

        let decoded = rest
            .find(';')
            .filter(|end| *end <= 10)
            .and_then(|end| decode_entity(&rest[1..end]).map(|character| (character, end + 1)));

        match decoded {
            Some((character, consumed)) => {
                result.push(character);
                rest = &rest[consumed..];
            }
            None => {
                result.push('&');
                rest = &rest[1..];
            }
        }
    }

    result.push_str(rest);
    result
}

fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = entity.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MAX_ATTEMPTS, assemble, build_request_body, chunk_ranges, escape_html, parse_response,
        plan_cues, read_api_key, translate_cues, unescape_html,
    };
    use crate::interfaces::translate::Translator;
    use crate::srt::Cue;
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
        let body = r#"[["Eu n&#39;ao sei","<i>Eca!</i>","Tom &amp; Jerry &quot;ok&quot;"],["en","en","en"]]"#;
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
    fn reads_the_api_key_and_treats_missing_or_blank_values_as_absent() {
        assert_eq!(
            read_api_key(Some(" abc123\n".to_string())).as_deref(),
            Some("abc123")
        );
        assert_eq!(read_api_key(None), None);
        assert_eq!(read_api_key(Some("   ".to_string())), None);
    }

    #[test]
    fn parses_gtx_segments_into_one_line_per_input() {
        let body = r#"[[["É realmente interessante\n","It's really interesting\n",null,null,10],["para ver quem você se torna\n","to see who you become\n"],["Eca!","Ugh!"]],null,"en"]"#;

        assert_eq!(
            super::parse_gtx_response(body, 3).unwrap(),
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

        assert!(super::parse_gtx_response(body, 3).is_err());
        assert!(super::parse_gtx_response("not json", 1).is_err());
        assert!(super::parse_gtx_response(r#"{"a":1}"#, 1).is_err());
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

        let result =
            translate_cues(&translator, &[cue("1", "hi")], "en", "pt", Duration::ZERO).await;

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
}

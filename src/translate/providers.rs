use super::text::unescape_html;
use crate::interfaces::translate::Translator;
use rust_i18n::t;
use serde_json::{Value, json};

const ENDPOINT: &str = "https://translate-pa.googleapis.com/v1/translateHtml";
const GTX_ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";

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

pub(super) fn parse_gtx_response(body: &str, expected: usize) -> Result<Vec<String>, String> {
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

pub(super) fn build_request_body(texts: &[String], source: &str, target: &str) -> String {
    json!([[texts, source, target], "te"]).to_string()
}

pub(super) fn parse_response(body: &str, expected: usize) -> Result<Vec<String>, String> {
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

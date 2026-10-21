use super::text::escape_html;
use crate::interfaces::translate::Translator;
use crate::srt::{self, Cue};
use futures_util::{StreamExt, TryStreamExt, stream};
use std::{ops::Range, time::Duration};

const MAX_BATCH_ITEMS: usize = 100;
const MAX_BATCH_CHARS: usize = 4000;
const MAX_CONCURRENT_REQUESTS: usize = 4;
pub(super) const MAX_ATTEMPTS: u32 = 3;
pub(super) const RETRY_DELAY: Duration = Duration::from_millis(500);
const MAX_LINE_CHARS: usize = 42;

#[cfg(test)]
pub(super) async fn translate_cues<T: Translator>(
    translator: &T,
    cues: &[Cue],
    source: &str,
    target: &str,
    retry_delay: Duration,
) -> Result<Vec<Cue>, String> {
    translate_cues_with_progress(translator, cues, source, target, retry_delay, &mut |_| {}).await
}

pub async fn translate_cues_with_progress<T: Translator>(
    translator: &T,
    cues: &[Cue],
    source: &str,
    target: &str,
    retry_delay: Duration,
    on_progress: &mut dyn FnMut(u8),
) -> Result<Vec<Cue>, String> {
    let (plans, texts) = plan_cues(cues);
    let ranges = chunk_ranges(&texts, MAX_BATCH_ITEMS, MAX_BATCH_CHARS);
    let total_batches = ranges.len();
    let mut completed_batches = 0;
    if total_batches == 0 {
        on_progress(100);
    }

    let batches: Vec<Vec<String>> = stream::iter(ranges)
        .map(|range| translate_with_retry(translator, &texts[range], source, target, retry_delay))
        .buffered(MAX_CONCURRENT_REQUESTS)
        .map(|result| {
            if result.is_ok() {
                completed_batches += 1;
                on_progress(
                    (completed_batches * 100 / total_batches)
                        .min(100)
                        .try_into()
                        .unwrap_or(100),
                );
            }
            result
        })
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

pub(super) async fn translate_with_retry<T: Translator>(
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

pub(super) struct CuePlan {
    pub(super) prefix: String,
    pub(super) dialogue: bool,
    pub(super) segments: usize,
}

pub(super) fn plan_cues(cues: &[Cue]) -> (Vec<CuePlan>, Vec<String>) {
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

pub(super) fn assemble(cue: &Cue, plan: &CuePlan, translated: &[String]) -> Cue {
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

pub(super) fn chunk_ranges(
    texts: &[String],
    max_items: usize,
    max_chars: usize,
) -> Vec<Range<usize>> {
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

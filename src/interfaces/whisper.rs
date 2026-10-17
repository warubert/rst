use crate::languages::SupportedLanguage;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranscriptionEvent {
    Progress(u8),
    CpuFallback,
}

pub trait SubtitleTranscriber {
    fn supported_languages(&self) -> Result<Vec<SupportedLanguage>, String>;

    fn transcribe(
        &self,
        input: &str,
        language: &str,
        output: &str,
        on_event: &mut dyn FnMut(TranscriptionEvent),
    ) -> Result<PathBuf, String>;
}

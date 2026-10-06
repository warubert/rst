#[derive(Debug)]
pub struct Subtitle {
    pub index: i64,
    pub language: String,
    pub is_sdh: bool,
}

pub trait SubtitleProbe {
    fn get_subtitle_streams(&self, input: &str) -> Result<Vec<Subtitle>, String>;
}

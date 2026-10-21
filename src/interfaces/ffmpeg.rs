pub trait SubtitleExtractor {
    fn extract_subtitle(
        &self,
        input: &str,
        subtitle_index: i64,
        output_srt: &str,
    ) -> Result<(), String>;

    fn extract_subtitle_with_progress(
        &self,
        input: &str,
        subtitle_index: i64,
        output_srt: &str,
        on_progress: &mut dyn FnMut(u8),
    ) -> Result<(), String> {
        on_progress(0);
        self.extract_subtitle(input, subtitle_index, output_srt)?;
        on_progress(100);
        Ok(())
    }
}

use std::future::Future;

pub trait Translator {
    fn translate_batch(
        &self,
        texts: &[String],
        source: &str,
        target: &str,
    ) -> impl Future<Output = Result<Vec<String>, String>>;
}

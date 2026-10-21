// Infrastructure: Text processor

use regex::Regex;

/// Text processor for filtering, normalization, and replacement
pub struct TextProcessor {
    placeholder_pattern: Regex,
    placeholder_store: std::collections::HashMap<String, String>,
}

impl TextProcessor {
    pub fn new() -> Self {
        Self {
            placeholder_pattern: Regex::new(r"\[placeholder:([a-zA-Z0-9_]+)\]").unwrap(),
            placeholder_store: std::collections::HashMap::new(),
        }
    }

    /// Preprocess text: extract placeholders, filter, normalize
    pub fn preprocess(&mut self, text: &str) -> String {
        let text = self.remove_zero_width_chars(text);
        let text = self.extract_placeholders(&text);
        text
    }

    /// Postprocess text: restore placeholders
    pub fn postprocess(&mut self, text: &str) -> String {
        self.restore_placeholders(text)
    }

    /// Remove zero-width characters
    fn remove_zero_width_chars(&self, text: &str) -> String {
        text.replace('\u{200B}', "")
            .replace('\u{200C}', "")
            .replace('\u{200D}', "")
            .replace('\u{FEFF}', "")
    }

    /// Extract placeholders for safe processing
    fn extract_placeholders(&mut self, text: &str) -> String {
        self.placeholder_store.clear();
        let mut counter = 0;
        self.placeholder_pattern.replace_all(text, |caps: &regex::Captures| {
            let placeholder = format!("[placeholder:ph_{}]", counter);
            self.placeholder_store.insert(placeholder.clone(), caps[0].to_string());
            counter += 1;
            placeholder
        }).into_owned()
    }

    /// Restore placeholders to original values
    fn restore_placeholders(&mut self, text: &str) -> String {
        let result = self.placeholder_pattern.replace_all(text, |caps: &regex::Captures| {
            self.placeholder_store.get(&caps[0]).cloned().unwrap_or_else(|| caps[0].to_string())
        }).into_owned();
        self.placeholder_store.clear();
        result
    }
}

#[cfg(test)]
mod tests {
    use super::TextProcessor;

    #[test]
    fn placeholder_roundtrip_does_not_rescan_generated_placeholders() {
        let mut processor = TextProcessor::new();
        let original = "Hello [placeholder:user] and [placeholder:ph_0]";
        let processed = processor.preprocess(original);
        assert_eq!(processor.postprocess(&processed), original);
    }
}

impl Default for TextProcessor {
    fn default() -> Self {
        Self::new()
    }
}

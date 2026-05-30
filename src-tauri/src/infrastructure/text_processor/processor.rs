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
        let mut result = text.to_string();
        let mut counter = 0;

        while let Some(m) = self.placeholder_pattern.find(&result) {
            let placeholder = format!("[placeholder:ph_{}]", counter);
            self.placeholder_store.insert(placeholder.clone(), m.as_str().to_string());
            result.replace_range(m.range(), &placeholder);
            counter += 1;
        }

        result
    }

    /// Restore placeholders to original values
    fn restore_placeholders(&mut self, text: &str) -> String {
        let mut result = text.to_string();

        for (placeholder, original) in &self.placeholder_store {
            result = result.replace(placeholder, original);
        }

        self.placeholder_store.clear();
        result
    }
}

impl Default for TextProcessor {
    fn default() -> Self {
        Self::new()
    }
}
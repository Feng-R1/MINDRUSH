// Domain entity: Cache Item

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};
use uuid::Uuid;

/// Translation status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum TranslationStatus {
    Untranslated = 0,
    Translated = 1,
    Polished = 2,
    Excluded = 7,
}

impl Default for TranslationStatus {
    fn default() -> Self {
        TranslationStatus::Untranslated
    }
}

/// Cache item - stores translation result for a single text
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheItem {
    pub id: Uuid,
    pub text_index: usize,
    pub translation_status: TranslationStatus,
    pub model: String,
    pub source_text: String,
    pub translated_text: String,
    pub text_to_detect: Option<String>,
    pub lang_code: Option<(String, f32, Vec<String>)>,
    pub extra: std::collections::HashMap<String, String>,
    pub source_hash: String,
    pub tokens_used: u32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl CacheItem {
    pub fn new(source_text: String, text_index: usize) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            text_index,
            translation_status: TranslationStatus::Untranslated,
            model: String::new(),
            source_text: source_text.clone(),
            translated_text: String::new(),
            text_to_detect: None,
            lang_code: None,
            extra: std::collections::HashMap::new(),
            source_hash: Self::compute_hash(&source_text),
            tokens_used: 0,
            created_at: now,
            updated_at: now,
        }
    }

    /// Compute SHA256 hash of source text for cache key
    pub fn compute_hash(text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Get final text (translated优先于source)
    pub fn final_text(&self) -> &str {
        if !self.translated_text.is_empty() {
            &self.translated_text
        } else {
            &self.source_text
        }
    }

    pub fn is_translated(&self) -> bool {
        matches!(self.translation_status, TranslationStatus::Translated | TranslationStatus::Polished)
    }

    pub fn is_excluded(&self) -> bool {
        matches!(self.translation_status, TranslationStatus::Excluded)
    }
}
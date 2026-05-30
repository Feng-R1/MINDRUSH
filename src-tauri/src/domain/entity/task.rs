// Domain entity: Translation Task

use chrono::{DateTime, Utc};
use uuid::Uuid;
use super::language::Language;

/// Language pair
#[derive(Debug, Clone, Copy)]
pub struct LanguagePair {
    pub source: Language,
    pub target: Language,
}

/// Task status
#[derive(Debug, Clone, PartialEq)]
pub enum TaskStatus {
    Pending,
    InProgress,
    Translated,
    Polished,
    Proofread,
    Failed(String),
}

/// Task metadata
#[derive(Debug, Clone, Default)]
pub struct TaskMetadata {
    pub file_path: Option<String>,
    pub line_number: Option<usize>,
    pub context_before: Vec<String>,
    pub context_after: Vec<String>,
    pub character_name: Option<String>,
}

/// Translation task entity
#[derive(Debug, Clone)]
pub struct Task {
    pub id: Uuid,
    pub source_text: String,
    pub translated_text: Option<String>,
    pub status: TaskStatus,
    pub language_pair: LanguagePair,
    pub metadata: TaskMetadata,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Task {
    pub fn new(source_text: String, language_pair: LanguagePair) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            source_text,
            translated_text: None,
            status: TaskStatus::Pending,
            language_pair,
            metadata: TaskMetadata::default(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_metadata(mut self, metadata: TaskMetadata) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn mark_translated(&mut self, text: String) {
        self.translated_text = Some(text);
        self.status = TaskStatus::Translated;
        self.updated_at = Utc::now();
    }

    pub fn mark_failed(&mut self, error: String) {
        self.status = TaskStatus::Failed(error);
        self.updated_at = Utc::now();
    }
}
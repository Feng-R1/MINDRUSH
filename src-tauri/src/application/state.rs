// Application state management

use crate::application::commands::history::HistoryItem;
use crate::infrastructure::llm::{LLMConfig, LLMConfigEntry};
use crate::infrastructure::cache::MemoryCache;
use crate::infrastructure::text_processor::TextProcessor;
use crate::domain::service::CacheService;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SegmentType {
    Paragraph,
    Heading { level: u8 },
    Code,
    ListItem,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SegmentMeta {
    pub segment_type: SegmentType,
    pub keep_original: bool,
    #[serde(default)]
    pub font_size: Option<f32>,
    #[serde(default)]
    pub is_monospace: bool,
    #[serde(default)]
    pub indent: Option<f32>,
    #[serde(default)]
    pub is_italic: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpubLoadInfo {
    pub filename: String,
    pub segment_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarkedSection {
    /// EPUB internal path, e.g. "OPS/ch1.xhtml"
    pub spine_path: String,
    /// XHTML with \x00EPUB_N\x00 markers instead of text
    pub marked_xhtml: String,
    /// Starting index of this section's segments in the flat segments array
    pub segment_start: usize,
    /// Number of segments in this section
    pub segment_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpubDocument {
    /// Original EPUB file path
    pub original_path: String,
    /// Filename for display
    pub filename: String,
    /// Original text segments (in spine order)
    pub segments: Vec<String>,
    /// Translated text (None = not yet translated)
    pub translations: Vec<Option<String>>,
    /// Per-segment metadata (type, formatting flags)
    #[serde(default)]
    pub segment_meta: Vec<SegmentMeta>,
    /// Sections with marked XHTML
    pub marked_sections: Vec<MarkedSection>,
    /// Inline code/Html content referenced by `C0` placeholders in segments
    pub code_spans: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PdfElement {
    TextSegment {
        text: String,
        meta: SegmentMeta,
        segment_idx: usize,
    },
    CodeBlock {
        text: String,
        font_size: f32,
    },
    Image {
        data: Vec<u8>,
        x: f32, y: f32,
        width: f32, height: f32,
        page: usize,
        caption: Option<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PdfDocumentData {
    pub original_path: String,
    pub filename: String,
    pub elements: Vec<PdfElement>,
    pub translatable_segments: Vec<String>,
    pub translations: Vec<Option<String>>,
}

/// Application state shared across all commands
pub struct AppState {
    pub llm_configs: RwLock<Vec<LLMConfigEntry>>,
    pub active_llm_index: RwLock<usize>,
    pub cache: Arc<RwLock<dyn CacheService>>,
    pub text_processor: RwLock<TextProcessor>,
    pub history: RwLock<Vec<HistoryItem>>,
    pub system_prompt: RwLock<String>,
    pub glossary: RwLock<String>,
    pub epub_document: RwLock<Option<EpubDocument>>,
    pub pdf_document: RwLock<Option<PdfDocumentData>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            llm_configs: RwLock::new(vec![LLMConfigEntry {
                name: "Default".to_string(),
                config: LLMConfig::default(),
            }]),
            active_llm_index: RwLock::new(0),
            cache: Arc::new(RwLock::new(MemoryCache::new())),
            text_processor: RwLock::new(TextProcessor::new()),
            history: RwLock::new(Vec::new()),
            system_prompt: RwLock::new("You are a professional translator. Translate the following text accurately.".to_string()),
            glossary: RwLock::new(String::new()),
            epub_document: RwLock::new(None),
            pdf_document: RwLock::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
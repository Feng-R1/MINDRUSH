// Domain service traits - interfaces for infrastructure layer

use crate::domain::entity::CacheItem;
use crate::domain::value_object::ValidationResult;

/// Cache service trait
pub trait CacheService: Send + Sync {
    fn get(&self, key: &str) -> Option<CacheItem>;
    fn set(&mut self, key: String, item: CacheItem);
    fn remove(&mut self, key: &str);
    fn clear(&mut self);
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

/// Translation domain service trait
pub trait TranslationDomainService: Send + Sync {
    fn should_translate(&self, text: &str, cache: &dyn CacheService) -> bool;
    fn compute_cache_key(&self, text: &str, model: &str) -> String;
    fn validate_translation(&self, original: &str, translated: &str) -> ValidationResult;
    fn preprocess_text(&self, text: &str) -> String;
}
// Infrastructure: In-memory cache implementation

use crate::domain::entity::CacheItem;

/// In-memory cache store for testing and development
pub struct MemoryCache {
    items: std::collections::HashMap<String, CacheItem>,
    hits: usize,
    misses: usize,
}

impl MemoryCache {
    pub fn new() -> Self {
        Self {
            items: std::collections::HashMap::new(),
            hits: 0,
            misses: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            items: std::collections::HashMap::with_capacity(capacity),
            hits: 0,
            misses: 0,
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

impl Default for MemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::domain::service::CacheService for MemoryCache {
    fn get(&self, key: &str) -> Option<CacheItem> {
        self.items.get(key).cloned()
    }

    fn set(&mut self, key: String, item: CacheItem) {
        self.items.insert(key, item);
    }

    fn remove(&mut self, key: &str) {
        self.items.remove(key);
    }

    fn clear(&mut self) {
        self.items.clear();
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}
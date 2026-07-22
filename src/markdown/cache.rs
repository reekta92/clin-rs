use std::sync::{Arc, LazyLock};
use std::hash::{Hash, Hasher};
use parking_lot::Mutex;
use super::style::{MarkdownTheme, RenderedDocument};
use super::MdRenderOpts;
use super::builtin::{HighlightedBlock, HighlightedSpan};

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct RenderKey {
    pub fingerprint: u64,
    pub content: Arc<str>,
    pub cols: u16,
    pub theme: MarkdownTheme,
    pub opts: MdRenderOpts,
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct HighlightKey {
    pub fingerprint: u64,
    pub language: Arc<str>,
    pub code_theme: Arc<str>,
    pub literal: Arc<str>,
}

impl Hash for RenderKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
    }
}

impl Hash for HighlightKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
    }
}

impl RenderKey {
    pub fn new(content: Arc<str>, cols: u16, theme: MarkdownTheme, opts: MdRenderOpts) -> Self {
        let mut hasher = std::hash::DefaultHasher::new();
        content.hash(&mut hasher);
        cols.hash(&mut hasher);
        theme.hash(&mut hasher);
        opts.hash(&mut hasher);
        let fingerprint = hasher.finish();
        Self {
            fingerprint,
            content,
            cols,
            theme,
            opts,
        }
    }
}

impl HighlightKey {
    pub fn new(language: Arc<str>, code_theme: Arc<str>, literal: Arc<str>, literal_fingerprint: u64) -> Self {
        let mut hasher = std::hash::DefaultHasher::new();
        language.hash(&mut hasher);
        code_theme.hash(&mut hasher);
        literal_fingerprint.hash(&mut hasher);
        let fingerprint = hasher.finish();
        Self {
            fingerprint,
            language,
            code_theme,
            literal,
        }
    }
}

pub(crate) struct ByteLru<K: Eq + Hash, V> {
    cache: lru::LruCache<K, V>,
    current_bytes: usize,
    max_bytes: usize,
    max_entries: usize,
    size_of_k: fn(&K) -> usize,
    size_of_v: fn(&V) -> usize,
}

impl<K: Eq + Hash, V> ByteLru<K, V> {
    pub fn new(
        max_entries: usize,
        max_bytes: usize,
        size_of_k: fn(&K) -> usize,
        size_of_v: fn(&V) -> usize,
    ) -> Self {
        Self {
            cache: lru::LruCache::unbounded(),
            current_bytes: 0,
            max_bytes,
            max_entries,
            size_of_k,
            size_of_v,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        self.cache.get(key)
    }

    pub fn insert(&mut self, key: K, value: V) {
        let key_size = (self.size_of_k)(&key);
        let val_size = (self.size_of_v)(&value);
        let entry_size = key_size + val_size;

        if entry_size > self.max_bytes {
            if let Some(old_val) = self.cache.pop(&key) {
                self.current_bytes -= key_size + (self.size_of_v)(&old_val);
            }
            return;
        }

        if let Some(old_val) = self.cache.pop(&key) {
            self.current_bytes -= key_size + (self.size_of_v)(&old_val);
        }

        self.cache.put(key, value);
        self.current_bytes += entry_size;

        while self.cache.len() > self.max_entries || self.current_bytes > self.max_bytes {
            if let Some((k, v)) = self.cache.pop_lru() {
                self.current_bytes -= (self.size_of_k)(&k) + (self.size_of_v)(&v);
            } else {
                break;
            }
        }
    }

    pub fn clear(&mut self) {
        self.cache.clear();
        self.current_bytes = 0;
    }

    pub fn len(&self) -> usize {
        self.cache.len()
    }

    pub fn bytes(&self) -> usize {
        self.current_bytes
    }
}

fn render_key_size(k: &RenderKey) -> usize {
    k.content.len() + k.opts.code_theme.capacity() + std::mem::size_of::<RenderKey>()
}

fn document_size(v: &Arc<RenderedDocument>) -> usize {
    v.estimated_bytes()
}

fn highlight_key_size(k: &HighlightKey) -> usize {
    k.language.len() + k.code_theme.len() + k.literal.len() + std::mem::size_of::<HighlightKey>()
}

fn highlighted_block_size(block: &Arc<HighlightedBlock>) -> usize {
    let mut bytes = std::mem::size_of::<HighlightedBlock>() + block.capacity() * std::mem::size_of::<Option<Vec<HighlightedSpan>>>();
    for opt_line in &***block {
        if let Some(spans) = opt_line {
            bytes += std::mem::size_of::<Vec<HighlightedSpan>>() + spans.capacity() * std::mem::size_of::<HighlightedSpan>();
            for span in spans {
                bytes += std::mem::size_of::<HighlightedSpan>() + span.text.capacity();
            }
        }
    }
    bytes
}

static DOCUMENT_CACHE: LazyLock<Mutex<ByteLru<RenderKey, Arc<RenderedDocument>>>> = LazyLock::new(|| {
    Mutex::new(ByteLru::new(32, 64 * 1024 * 1024, render_key_size, document_size))
});

static HIGHLIGHT_CACHE: LazyLock<Mutex<ByteLru<HighlightKey, Arc<HighlightedBlock>>>> = LazyLock::new(|| {
    Mutex::new(ByteLru::new(1024, 32 * 1024 * 1024, highlight_key_size, highlighted_block_size))
});

pub(crate) fn get_document(key: &RenderKey) -> Option<Arc<RenderedDocument>> {
    DOCUMENT_CACHE.lock().get(key).cloned()
}

pub(crate) fn insert_document(key: RenderKey, doc: Arc<RenderedDocument>) {
    DOCUMENT_CACHE.lock().insert(key, doc);
}

pub(crate) fn get_highlight(key: &HighlightKey) -> Option<Arc<HighlightedBlock>> {
    HIGHLIGHT_CACHE.lock().get(key).cloned()
}

pub(crate) fn insert_highlight(key: HighlightKey, block: Arc<HighlightedBlock>) {
    HIGHLIGHT_CACHE.lock().insert(key, block);
}

#[cfg(test)]
pub(crate) fn clear_markdown_caches() {
    DOCUMENT_CACHE.lock().clear();
    HIGHLIGHT_CACHE.lock().clear();
}

#[cfg(test)]
pub(crate) fn cache_stats() -> (usize, usize, usize, usize) {
    let doc = DOCUMENT_CACHE.lock();
    let hl = HIGHLIGHT_CACHE.lock();
    (doc.len(), doc.bytes(), hl.len(), hl.bytes())
}

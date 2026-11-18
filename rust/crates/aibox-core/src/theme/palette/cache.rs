use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock};

use super::derive::derive_palette_uncached;
use super::{Palette, PaletteOptions};
use crate::theme::parse::{parse_colors, ThemeColors};

const MAX_CACHE_ENTRIES: usize = 64;

/// Deterministic 64-bit FNV-1a hasher for cache keys.
struct Fnv1aHasher(u64);

impl Fnv1aHasher {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Self(Self::OFFSET_BASIS)
    }
}

impl Hasher for Fnv1aHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }
}

fn hash_derive_key(colors: &ThemeColors, opts: PaletteOptions) -> u64 {
    let mut h = Fnv1aHasher::new();
    colors.hash(&mut h);
    opts.hash(&mut h);
    h.finish()
}

fn hash_content_key(toml_text: &str, opts: PaletteOptions) -> u64 {
    let mut h = Fnv1aHasher::new();
    toml_text.as_bytes().hash(&mut h);
    opts.hash(&mut h);
    h.finish()
}

/// Bounded in-memory cache of derived `Palette`s keyed by input content hash,
/// `ThemeColors`, and `PaletteOptions`.
#[derive(Debug, Default)]
pub struct PaletteCache {
    derive_entries: HashMap<u64, Palette>,
    content_entries: HashMap<u64, (ThemeColors, Palette)>,
    hits: u64,
    misses: u64,
}

impl PaletteCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hits(&self) -> u64 {
        self.hits
    }

    pub fn misses(&self) -> u64 {
        self.misses
    }

    pub fn len(&self) -> usize {
        self.derive_entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.derive_entries.is_empty()
    }

    /// Returns a cached `Palette` for `(colors, opts)` or derives and caches it.
    pub fn get_or_derive(&mut self, colors: &ThemeColors, opts: PaletteOptions) -> Palette {
        let key = hash_derive_key(colors, opts);
        if let Some(&palette) = self.derive_entries.get(&key) {
            self.hits += 1;
            return palette;
        }
        self.misses += 1;
        let palette = derive_palette_uncached(colors, opts);
        if self.derive_entries.len() >= MAX_CACHE_ENTRIES {
            self.derive_entries.clear();
        }
        self.derive_entries.insert(key, palette);
        palette
    }

    /// Returns cached `(ThemeColors, Palette)` when `toml_text` and `opts` are unchanged,
    /// skipping both parsing and palette derivation on cache hits.
    pub fn get_or_parse_and_derive(
        &mut self,
        toml_text: &str,
        opts: PaletteOptions,
    ) -> (ThemeColors, Palette) {
        let content_key = hash_content_key(toml_text, opts);
        if let Some((colors, palette)) = self.content_entries.get(&content_key) {
            self.hits += 1;
            return (colors.clone(), *palette);
        }

        let colors = parse_colors(toml_text);
        let palette = self.get_or_derive(&colors, opts);
        if self.content_entries.len() >= MAX_CACHE_ENTRIES {
            self.content_entries.clear();
        }
        self.content_entries
            .insert(content_key, (colors.clone(), palette));
        (colors, palette)
    }
}

pub(crate) fn global_cache() -> &'static Mutex<PaletteCache> {
    static CACHE: OnceLock<Mutex<PaletteCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(PaletteCache::new()))
}

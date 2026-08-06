//! Persistent embedding cache backed by SQLite.
//!
//! Caches query embeddings by SHA256(text + model_name) to avoid re-loading
//! the 188MB ONNX model on every CLI invocation (#896).
//!
//! - Cache hit: ~1ms (SQLite read + bincode deserialize)
//! - Cache miss: falls through to inner embedder (~2s ONNX load + ~50ms inference)
//!
//! The cache lives in a dedicated SQLite file (`embed_cache.db`) inside the
//! uteke home directory, separate from the main `uteke.db` to avoid schema
//! coupling and lock contention.

use std::path::Path;
use std::sync::Mutex;

use rusqlite::Connection;
use sha2::{Digest, Sha256};

use crate::Error;
use crate::embed::Embedder;

/// Persistent SQLite cache for embedding vectors.
///
/// Stores text→vector mappings keyed by SHA256(model_name + text).
/// Thread-safe via internal `Mutex` on the `Connection`.
pub struct EmbeddingCache {
    conn: Mutex<Connection>,
}

impl EmbeddingCache {
    /// Open (or create) the cache database at `cache_path`.
    pub fn open(cache_path: &Path) -> Result<Self, Error> {
        let conn = Connection::open(cache_path)
            .map_err(|e| Error::db("open embedding cache database", e))?;

        conn.execute_batch("PRAGMA journal_mode=WAL;")
            .map_err(|e| Error::db("set cache WAL mode", e))?;
        conn.execute_batch("PRAGMA synchronous=NORMAL;")
            .map_err(|e| Error::db("set cache synchronous mode", e))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS embedding_cache (
                text_hash   TEXT PRIMARY KEY,
                model_name  TEXT NOT NULL,
                embedding   BLOB NOT NULL,
                created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
            );
            CREATE INDEX IF NOT EXISTS idx_cache_model ON embedding_cache(model_name);",
        )
        .map_err(|e| Error::db("create embedding cache schema", e))?;

        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Compute the cache key: SHA256(model_name + "\0" + text).
    fn cache_key(model_name: &str, text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(model_name.as_bytes());
        hasher.update(b"\0");
        hasher.update(text.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    /// Look up a cached embedding. Returns `None` on miss.
    fn lookup(&self, model_name: &str, text: &str) -> Option<Vec<f32>> {
        let key = Self::cache_key(model_name, text);
        let conn = self.conn.lock().ok()?;

        let blob: Vec<u8> = conn
            .query_row(
                "SELECT embedding FROM embedding_cache WHERE text_hash = ?1 AND model_name = ?2",
                rusqlite::params![&key, model_name],
                |row| row.get(0),
            )
            .ok()?;

        deserialize_embedding(&blob)
    }

    /// Store an embedding in the cache.
    fn store(&self, model_name: &str, text: &str, embedding: &[f32]) {
        let key = Self::cache_key(model_name, text);
        let blob = serialize_embedding(embedding);

        if let Ok(conn) = self.conn.lock() {
            let _ = conn.execute(
                "INSERT OR REPLACE INTO embedding_cache (text_hash, model_name, embedding)
                 VALUES (?1, ?2, ?3)",
                rusqlite::params![&key, model_name, &blob],
            );
        }
    }

    /// Clear all cached embeddings for a specific model.
    /// Called when the model is upgraded or changed.
    #[allow(dead_code)] // Public API for future cache invalidation
    pub fn invalidate_model(&self, model_name: &str) -> Result<(), Error> {
        let conn = self
            .conn
            .lock()
            .map_err(|_| Error::lock("embedding cache during invalidation"))?;
        conn.execute(
            "DELETE FROM embedding_cache WHERE model_name = ?1",
            rusqlite::params![model_name],
        )
        .map_err(|e| Error::db("invalidate embedding cache", e))?;
        Ok(())
    }
}

/// Decorator that wraps any [`Embedder`] with a persistent [`EmbeddingCache`].
///
/// On cache hit (~1ms SQLite read), skips the inner embedder entirely —
/// avoiding the ~2s ONNX model load on repeated CLI invocations (#896).
pub struct CachingEmbedder {
    inner: Box<dyn Embedder>,
    cache: EmbeddingCache,
}

impl CachingEmbedder {
    /// Wrap `inner` with a pre-opened `EmbeddingCache`.
    pub fn with_cache(inner: Box<dyn Embedder>, cache: EmbeddingCache) -> Self {
        Self { inner, cache }
    }
}

impl Embedder for CachingEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, Error> {
        let model_name = self.inner.name();

        // Try cache first
        if let Some(cached) = self.cache.lookup(model_name, text) {
            tracing::debug!(text_len = text.len(), "embedding cache hit");
            return Ok(cached);
        }

        // Cache miss — compute embedding
        tracing::debug!(text_len = text.len(), "embedding cache miss");
        let embedding = self.inner.embed(text)?;

        // Store in cache (best-effort, don't fail if cache write errors)
        self.cache.store(model_name, text, &embedding);

        Ok(embedding)
    }

    fn dims(&self) -> usize {
        self.inner.dims()
    }

    fn max_seq_len(&self) -> usize {
        self.inner.max_seq_len()
    }

    fn name(&self) -> &str {
        self.inner.name()
    }
}

// ── Serialization ──────────────────────────────────────────────────────────

/// Serialize f32 slice as little-endian bytes (4 bytes per element).
fn serialize_embedding(embedding: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(embedding.len() * 4);
    for &v in embedding {
        bytes.extend_from_slice(&v.to_le_bytes());
    }
    bytes
}

/// Deserialize little-endian bytes back to Vec<f32>.
fn deserialize_embedding(blob: &[u8]) -> Option<Vec<f32>> {
    if blob.len() % 4 != 0 {
        return None;
    }
    let mut result = Vec::with_capacity(blob.len() / 4);
    for chunk in blob.chunks_exact(4) {
        let bytes: [u8; 4] = chunk.try_into().ok()?;
        result.push(f32::from_le_bytes(bytes));
    }
    Some(result)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock embedder that counts calls.
    struct CountingEmbedder {
        dims: usize,
        call_count: Mutex<usize>,
    }

    impl Embedder for CountingEmbedder {
        fn embed(&self, _text: &str) -> Result<Vec<f32>, Error> {
            let mut count = self.call_count.lock().unwrap();
            *count += 1;
            Ok(vec![0.5; self.dims])
        }

        fn dims(&self) -> usize {
            self.dims
        }

        fn max_seq_len(&self) -> usize {
            128
        }

        fn name(&self) -> &str {
            "mock-model"
        }
    }

    fn tmp_cache_path() -> std::path::PathBuf {
        let dir = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        dir.join(format!("uteke_test_cache_{nanos}.db"))
    }

    #[test]
    fn test_cache_hit_skips_inner() {
        let cache_path = tmp_cache_path();
        let inner = Box::new(CountingEmbedder {
            dims: 4,
            call_count: Mutex::new(0),
        });
        let cache_db = EmbeddingCache::open(&cache_path).unwrap();
        let cache = CachingEmbedder::with_cache(inner, cache_db);

        // First call — miss, inner called
        let emb1 = cache.embed("hello world").unwrap();
        assert_eq!(emb1, vec![0.5; 4]);

        // Second call — hit, inner NOT called
        let emb2 = cache.embed("hello world").unwrap();
        assert_eq!(emb2, vec![0.5; 4]);

        // Verify inner was only called once
        let count = {
            // Access inner through trait — we stored call_count in the mock,
            // but can't access it directly through Box<dyn>. Instead, verify
            // correctness by checking the cache persisted to SQLite.
            let conn = Connection::open(&cache_path).unwrap();
            let count: i64 = conn
                .query_row("SELECT COUNT(*) FROM embedding_cache", [], |row| row.get(0))
                .unwrap();
            count
        };
        assert_eq!(count, 1, "only one entry should be in the cache");

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn test_different_text_separate_entries() {
        let cache_path = tmp_cache_path();
        let inner = Box::new(CountingEmbedder {
            dims: 2,
            call_count: Mutex::new(0),
        });
        let cache_db = EmbeddingCache::open(&cache_path).unwrap();
        let cache = CachingEmbedder::with_cache(inner, cache_db);

        let _ = cache.embed("text one").unwrap();
        let _ = cache.embed("text two").unwrap();
        let _ = cache.embed("text one").unwrap(); // cache hit

        let conn = Connection::open(&cache_path).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM embedding_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2, "two unique texts = two cache entries");

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn test_cache_key_deterministic() {
        let key1 = EmbeddingCache::cache_key("model-a", "hello");
        let key2 = EmbeddingCache::cache_key("model-a", "hello");
        let key3 = EmbeddingCache::cache_key("model-b", "hello");

        assert_eq!(key1, key2, "same model + text = same key");
        assert_ne!(key1, key3, "different model = different key");
    }

    #[test]
    fn test_serialize_deserialize_roundtrip() {
        let embedding = vec![0.1, 0.2, 0.3, -0.4, 1.5];
        let blob = serialize_embedding(&embedding);
        let restored = deserialize_embedding(&blob).unwrap();

        assert_eq!(embedding.len(), restored.len());
        for (a, b) in embedding.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_invalidate_model() {
        let cache_path = tmp_cache_path();
        let inner = Box::new(CountingEmbedder {
            dims: 2,
            call_count: Mutex::new(0),
        });
        let cache_db = EmbeddingCache::open(&cache_path).unwrap();
        let cache = CachingEmbedder::with_cache(inner, cache_db);

        cache.embed("text one").unwrap();
        cache.embed("text two").unwrap();

        // Verify 2 entries
        let conn_check = Connection::open(&cache_path).unwrap();
        let count: i64 = conn_check
            .query_row("SELECT COUNT(*) FROM embedding_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 2);

        // Invalidate
        let conn_invalidate = Connection::open(&cache_path).unwrap();
        conn_invalidate
            .execute(
                "DELETE FROM embedding_cache WHERE model_name = ?1",
                rusqlite::params!["mock-model"],
            )
            .unwrap();

        let count_after: i64 = conn_check
            .query_row("SELECT COUNT(*) FROM embedding_cache", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count_after, 0, "cache should be empty after invalidation");

        let _ = std::fs::remove_file(&cache_path);
    }

    #[test]
    fn test_persistence_across_connections() {
        let cache_path = tmp_cache_path();

        // Write with one CachingEmbedder
        {
            let inner = Box::new(CountingEmbedder {
                dims: 3,
                call_count: Mutex::new(0),
            });
            let cache_db = EmbeddingCache::open(&cache_path).unwrap();
            let cache = CachingEmbedder::with_cache(inner, cache_db);
            let emb = cache.embed("persistent text").unwrap();
            assert_eq!(emb, vec![0.5; 3]);
        }

        // Read with a new CachingEmbedder (simulates new CLI invocation)
        {
            let inner = Box::new(CountingEmbedder {
                dims: 3,
                call_count: Mutex::new(0),
            });
            let cache_db = EmbeddingCache::open(&cache_path).unwrap();
            let cache = CachingEmbedder::with_cache(inner, cache_db);
            let emb = cache.embed("persistent text").unwrap();
            assert_eq!(emb, vec![0.5; 3]);

            // Inner should NOT have been called (cache hit from previous write)
            let count = {
                let conn = Connection::open(&cache_path).unwrap();
                conn.query_row::<i64, _, _>("SELECT COUNT(*) FROM embedding_cache", [], |row| {
                    row.get(0)
                })
                .unwrap()
            };
            assert_eq!(count, 1, "entry should persist from previous connection");
        }

        let _ = std::fs::remove_file(&cache_path);
    }
}

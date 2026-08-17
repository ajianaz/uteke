//! Import and export memories in JSONL format.

use crate::error::Error;
use crate::memory::types::{DEFAULT_NAMESPACE, ExportEntry, ImportResult, Memory};

impl crate::Uteke {
    /// Export all memories to JSONL format (one JSON object per line).
    ///
    /// Embeddings are NOT exported — they will be re-computed on import.
    /// This keeps export files small and portable.
    pub fn export(&self, namespace: Option<&str>) -> Result<String, Error> {
        let memories = self.store.load_all(namespace)?;
        let entries: Vec<ExportEntry> = memories
            .into_iter()
            .map(|m| ExportEntry {
                content: m.content,
                tags: m.tags,
                metadata: m.metadata,
                created_at: m.created_at,
                source: m.source,
            })
            .collect();

        let mut lines = Vec::with_capacity(entries.len());
        for entry in &entries {
            let line =
                serde_json::to_string(entry).map_err(|e| Error::db("export serialization", e))?;
            lines.push(line);
        }

        Ok(lines.join("\n"))
    }

    /// Import memories from JSONL or JSON array format.
    ///
    /// Accepts:
    /// - JSONL: one JSON object per line (each must have `content`)
    /// - JSON array: `[{"content":"..."}, ...]`
    /// - Single JSON object: `{"content":"..."}`
    ///
    /// Required field: `content`. Optional fields default automatically.
    /// Embeddings are re-computed during import.
    pub fn import(&self, input: &str, namespace: Option<&str>) -> Result<ImportResult, Error> {
        let trimmed = input.trim();

        // Detect format: JSON array, single JSON object, or JSONL.
        // Strategy:
        //   1. starts_with('[') → JSON array
        //   2. starts_with('{') → try single JSON object; if that fails, fall back to JSONL
        //      (handles both JSONL lines starting with `{` and pretty-printed objects)
        //   3. otherwise → JSONL
        let (entries, mut skipped): (Vec<ExportEntry>, usize) = if trimmed.starts_with('[') {
            match serde_json::from_str::<Vec<ExportEntry>>(trimmed) {
                Ok(arr) => (arr, 0),
                Err(e) => {
                    return Err(Error::validation(format!("Invalid JSON array: {e}")));
                }
            }
        } else if trimmed.starts_with('{') {
            // Try single JSON object first (covers both compact and pretty-printed)
            match serde_json::from_str::<ExportEntry>(trimmed) {
                Ok(entry) => (vec![entry], 0),
                Err(_) => {
                    // Fall back to JSONL: each line should be a self-contained JSON object
                    Self::parse_jsonl(input)
                }
            }
        } else {
            Self::parse_jsonl(input)
        };

        let mut imported = 0;
        let mut deduped = 0;

        for entry in entries {
            if entry.content.is_empty() {
                skipped += 1;
                continue;
            }

            // Re-embed the content with retry (consistent with remember path, #1005).
            self.ensure_embedder()?;
            let embedding = crate::operations::retry_embed(&self.embedder, &entry.content)?;

            // Dedup check: skip if cosine >= 0.95 to existing memory (#1005).
            if let Some(_existing_id) = self.check_duplicate(&embedding, namespace)? {
                deduped += 1;
                continue;
            }

            let id = uuid::Uuid::now_v7().to_string();
            let now = chrono::Utc::now();

            let memory = Memory {
                id: id.clone(),
                content: entry.content,
                embedding: embedding.clone(),
                tags: entry.tags,
                metadata: entry.metadata,
                created_at: entry.created_at,
                updated_at: now,
                namespace: namespace.unwrap_or(DEFAULT_NAMESPACE).to_string(),
                access_count: 0,
                last_accessed: None,
                deprecated: false,
                valid_from: Some(entry.created_at),
                valid_until: None,
                memory_type: "fact".to_string(),
                importance: 0.5,
                pinned: false,
                content_type: "text".to_string(),
                slug: None,
                source: Some(format!("import:{}", entry.source.unwrap_or_default())),
                source_type: "import".to_string(),
            };

            // Write-ahead: vector index first (can be rolled back), then SQLite.
            {
                let mut index = self
                    .index
                    .write()
                    .map_err(|_| Error::lock("index write lock during import"))?;
                index.insert(&id, &embedding)?;
                // Don't save per-item — we'll persist once after the full import.
            }

            if let Err(e) = self.store.insert(&memory) {
                // Rollback: remove from vector index
                let mut index = self
                    .index
                    .write()
                    .map_err(|_| Error::lock("index write lock during import rollback"))?;
                index.remove(&id);
                // Note: don't save per-entry — save once at end of import.
                // If process crashes, orphan entry is harmless and cleaned by repair.
                tracing::warn!("Skipping import entry (id={id}): {e}");
                skipped += 1;
                continue;
            }

            // Auto-link cosine edges (consistent with remember path, #1005).
            // Must run AFTER index.insert() so the new memory is searchable.
            self.auto_link_cosine(&id, &embedding, Some(memory.namespace.as_str()));

            imported += 1;
        }

        // Persist vector index after import completes
        if imported > 0 {
            let mut index = self
                .index
                .write()
                .map_err(|_| Error::lock("index write lock during import save"))?;
            index.save()?;
        }

        if skipped > 0 || deduped > 0 {
            tracing::info!(
                "Import completed: {imported} imported, {deduped} duplicates skipped, {skipped} errored entries."
            );
        }

        Ok(ImportResult { imported, skipped })
    }

    /// Parse JSONL input: one self-contained JSON object per line.
    /// Lines that fail to parse are counted as skipped (not fatal).
    fn parse_jsonl(input: &str) -> (Vec<ExportEntry>, usize) {
        let mut entries = Vec::new();
        let mut failed = 0;
        for line in input.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match serde_json::from_str::<ExportEntry>(line) {
                Ok(e) => entries.push(e),
                Err(e) => {
                    tracing::warn!("Skipping invalid JSONL line: {e}");
                    failed += 1;
                }
            }
        }
        (entries, failed)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_export_entry_serialization() {
        use crate::memory::types::ExportEntry;
        let entry = ExportEntry {
            content: "hello world".to_string(),
            tags: vec!["greeting".to_string()],
            metadata: serde_json::json!({}),
            created_at: chrono::Utc::now(),
            source: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let restored: ExportEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.content, "hello world");
        assert_eq!(restored.tags.len(), 1);
    }

    #[test]
    fn test_import_jsonl_multiple_lines() {
        let jsonl = r#"{"content":"first","tags":[],"metadata":{},"created_at":"2024-01-01T00:00:00Z","source":null}
{"content":"second","tags":[],"metadata":{},"created_at":"2024-01-01T00:00:00Z","source":null}"#;
        let (entries, failed) = crate::Uteke::parse_jsonl(jsonl);
        assert_eq!(entries.len(), 2);
        assert_eq!(failed, 0);
        assert_eq!(entries[0].content, "first");
        assert_eq!(entries[1].content, "second");
    }

    #[test]
    fn test_import_pretty_printed_single_object() {
        use crate::memory::types::ExportEntry;
        // Pretty-printed JSON object spanning multiple lines should parse as a single entry,
        // NOT fall through to JSONL line-by-line parsing.
        let pretty = r#"{
  "content": "hello pretty",
  "tags": ["test"],
  "metadata": {},
  "created_at": "2024-01-01T00:00:00Z",
  "source": null
}"#;
        // Simulate the import detection logic
        let trimmed = pretty.trim();
        assert!(trimmed.starts_with('{'));
        let result: Result<ExportEntry, _> = serde_json::from_str(trimmed);
        assert!(
            result.is_ok(),
            "Pretty-printed JSON should parse as single object"
        );
        assert_eq!(result.unwrap().content, "hello pretty");
    }
}

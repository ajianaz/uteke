//! Maintenance operations: doctor, verify, repair, stats, aging, prune, shutdown.

use crate::error::{Error, format_bytes};
use crate::memory::types::{AgingStatus, CleanupResult, Memory, PruneResult, StoreStats};
use crate::types::{
    DoctorCheck, DoctorReport, DoctorStatus, ReembedReport, RepairReport, VerifyReport,
};
use crate::uteke_home;

impl crate::Uteke {
    /// Check system health: DB, index, model, consistency.
    pub fn doctor(&self) -> Result<DoctorReport, Error> {
        let mut checks = Vec::new();

        // 1. SQLite DB
        let db_count = self.store.count(None)?;
        let db_path = self.store.path();
        let db_size = db_path
            .as_ref()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);
        checks.push(DoctorCheck {
            name: "SQLite DB".to_string(),
            status: DoctorStatus::Ok,
            detail: format!("{} memories, {}", db_count, format_bytes(db_size)),
        });

        // 2. usearch index
        let index = self
            .index
            .read()
            .map_err(|_| Error::lock("index read lock during doctor"))?;
        let index_count = index.len();
        checks.push(DoctorCheck {
            name: "usearch index".to_string(),
            status: DoctorStatus::Ok,
            detail: format!("{} vectors", index_count),
        });

        // 3. Index consistency
        if db_count == index_count {
            checks.push(DoctorCheck {
                name: "Index consistency".to_string(),
                status: DoctorStatus::Ok,
                detail: format!("DB={} Index={}", db_count, index_count),
            });
        } else {
            checks.push(DoctorCheck {
                name: "Index consistency".to_string(),
                status: DoctorStatus::Error,
                detail: format!(
                    "MISMATCH: DB={} Index={} — run `uteke repair`",
                    db_count, index_count
                ),
            });
        }

        // 4. Embedding model
        let model_dir = match uteke_home() {
            Ok(p) => p.join("models").join("embeddinggemma-q4"),
            Err(_) => {
                checks.push(DoctorCheck {
                    name: "Home directory".to_string(),
                    status: DoctorStatus::Error,
                    detail: "Cannot determine home directory. Set UTEKE_HOME.".to_string(),
                });
                return Ok(DoctorReport { checks });
            }
        };
        let model_file = model_dir.join("onnx").join("model_q4.onnx");
        let tokenizer_file = model_dir.join("tokenizer.json");
        let model_exists = model_file.exists() && tokenizer_file.exists();
        checks.push(DoctorCheck {
            name: "Embedding model".to_string(),
            status: if model_exists {
                DoctorStatus::Ok
            } else {
                DoctorStatus::Error
            },
            detail: if model_exists {
                "embeddinggemma-q4".to_string()
            } else {
                "Model files not found — will download on first use".to_string()
            },
        });

        Ok(DoctorReport { checks })
    }

    /// Verify DB and index consistency. Returns mismatch count.
    pub fn verify(&self) -> Result<VerifyReport, Error> {
        let db_count = self.store.count(None)?;
        let index = self
            .index
            .read()
            .map_err(|_| Error::lock("index read lock during verify"))?;
        let index_count = index.len();

        let consistent = db_count == index_count;
        Ok(VerifyReport {
            db_count,
            index_count,
            consistent,
        })
    }

    /// Repair: rebuild usearch index from SQLite + fix schema inconsistencies.
    pub fn repair(&self) -> Result<RepairReport, Error> {
        // Fix partially-migrated schema (e.g. missing has_children column, #500).
        self.store.ensure_schema_consistency()?;

        let before_db = self.store.count(None)?;
        let before_index = {
            let index = self
                .index
                .read()
                .map_err(|_| Error::lock("index read lock during repair (count before)"))?;
            index.len()
        };

        // Load all from SQLite and rebuild index
        let all_memories = self.store.load_all(None)?;
        let items: Vec<(String, Vec<f32>)> = all_memories
            .iter()
            .map(|m| (m.id.clone(), m.embedding.clone()))
            .collect();

        {
            let mut index = self
                .index
                .write()
                .map_err(|_| Error::lock("index write lock during repair (rebuild)"))?;
            index.build(&items)?;
            if let Err(e) = index.save() {
                tracing::warn!("Failed to save index: {e}");
            }
        }

        Ok(RepairReport {
            db_count: before_db,
            index_before: before_index,
            index_after: items.len(),
        })
    }

    /// Re-embed memories that have missing or empty embedding vectors.
    ///
    /// Scans all non-deprecated memories, finds those with empty embeddings,
    /// generates new embeddings, updates the database, and adds them to the index.
    pub fn reembed_missing(&self) -> Result<ReembedReport, Error> {
        let all_memories = self.store.load_all(None)?;
        let total_scanned = all_memories.len();

        // Filter to memories with empty embeddings, excluding deprecated.
        let missing: Vec<&Memory> = all_memories
            .iter()
            .filter(|m| !m.deprecated && m.embedding.is_empty())
            .collect();

        let missing_count = missing.len();
        if missing_count == 0 {
            return Ok(ReembedReport {
                total_scanned,
                missing_count,
                reembedded: 0,
                failed: 0,
            });
        }

        tracing::info!(
            total = total_scanned,
            missing = missing_count,
            "Re-embedding memories with missing vectors"
        );

        // Ensure embedder is initialized.
        self.ensure_embedder()?;

        let mut reembedded = 0usize;
        let mut failed = 0usize;
        let mut new_items: Vec<(String, Vec<f32>)> = Vec::new();

        // Acquire embedder lock ONCE before the loop to avoid
        // per-iteration mutex overhead (codecoradev/uteke#919 review).
        let embedder_guard = self
            .embedder
            .lock()
            .map_err(|_| Error::lock("embedder lock during reembed"))?;
        let embedder = embedder_guard
            .as_ref()
            .ok_or_else(|| Error::embed("reembed", "embedder not initialized"))?;

        for mem in &missing {
            match embedder.embed(&mem.content) {
                Ok(vec) => {
                    // Update memory in database.
                    let mut updated = (*mem).clone();
                    updated.embedding = vec.clone();
                    updated.updated_at = chrono::Utc::now();
                    if let Err(e) = self.store.update(&updated) {
                        tracing::warn!(id = %mem.id, error = %e, "Failed to update embedding in DB");
                        failed += 1;
                        continue;
                    }
                    new_items.push((mem.id.clone(), vec));
                    reembedded += 1;
                }
                Err(e) => {
                    tracing::warn!(id = %mem.id, error = %e, "Failed to generate embedding");
                    failed += 1;
                }
            }
        }

        // Add newly-embedded memories to the index.
        if !new_items.is_empty() {
            let mut index = self
                .index
                .write()
                .map_err(|_| Error::lock("index write during reembed"))?;
            for (id, vec) in &new_items {
                if let Err(e) = index.insert(id, vec) {
                    tracing::warn!(id = %id, error = %e, "Failed to add to index");
                }
            }
            if let Err(e) = index.save() {
                tracing::warn!(error = %e, "Failed to save index after reembed");
            }
        }

        tracing::info!(reembedded, failed, "Re-embed complete");

        Ok(ReembedReport {
            total_scanned,
            missing_count,
            reembedded,
            failed,
        })
    }

    /// Get statistics about the memory store.
    pub fn stats(&self, namespace: Option<&str>) -> Result<StoreStats, Error> {
        let total_memories = self.store.count(namespace)?;
        let unique_tags = self.store.unique_tags(namespace)?.len();
        let (hot, warm, cold) = self.store.tier_counts(
            namespace,
            self.tier_config.hot_days,
            self.tier_config.warm_days,
        )?;

        let db_size_bytes = self
            .store
            .path()
            .and_then(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .unwrap_or(0);

        let (cache_hits, cache_misses) = self.recall_cache.metrics();

        Ok(StoreStats {
            total_memories,
            unique_tags,
            db_size_bytes,
            hot,
            warm,
            cold,
            cache_hits,
            cache_misses,
        })
    }

    /// Get aging status — breakdown of memories by access tier.
    pub fn aging_status(&self, namespace: Option<&str>) -> Result<AgingStatus, Error> {
        let total = self.store.count(namespace)?;
        let (hot, warm, cold) = self.store.tier_counts(
            namespace,
            self.tier_config.hot_days,
            self.tier_config.warm_days,
        )?;
        let never_accessed = self.store.count_never_accessed(namespace)?;

        Ok(AgingStatus {
            total,
            hot,
            warm,
            cold,
            never_accessed,
        })
    }

    /// Preview aged memories eligible for cleanup (dry-run).
    pub fn aging_preview(
        &self,
        older_than_days: u32,
        max_access_count: u32,
        namespace: Option<&str>,
    ) -> Result<Vec<Memory>, Error> {
        self.store
            .find_aged(older_than_days, max_access_count, namespace)
    }

    /// Cleanup aged memories — deletes from SQLite AND removes from vector index.
    pub fn aging_cleanup(
        &self,
        older_than_days: u32,
        max_access_count: u32,
        namespace: Option<&str>,
    ) -> Result<CleanupResult, Error> {
        // Find aged memories first to get IDs for vector index removal
        let aged = self
            .store
            .find_aged(older_than_days, max_access_count, namespace)?;
        // Safety limit: never delete more than 100 memories per cycle.
        // This prevents mass deletion if thresholds are misconfigured.
        const MAX_DELETE_PER_CYCLE: usize = 100;
        let ids: Vec<String> = aged
            .into_iter()
            .take(MAX_DELETE_PER_CYCLE)
            .map(|m| m.id)
            .collect();

        if ids.is_empty() {
            return Ok(CleanupResult { deleted: 0 });
        }

        // Delete by specific IDs to avoid TOCTOU race (not re-query by criteria)
        let deleted = self.store.delete_by_ids(&ids)?;

        // Remove from vector index
        {
            let mut index = self
                .index
                .write()
                .map_err(|_| Error::lock("index write lock during aging_cleanup"))?;
            for id in &ids {
                index.remove(id);
            }
            if let Err(e) = index.save() {
                tracing::warn!("Failed to save index: {e}");
            }
        }

        Ok(CleanupResult { deleted })
    }

    /// Prune deprecated memories older than TTL days.
    ///
    /// Deletes from both SQLite and vector index.
    pub fn prune(
        &self,
        ttl_days: u32,
        namespace: Option<&str>,
        dry_run: bool,
    ) -> Result<PruneResult, Error> {
        let deprecated = self.store.find_deprecated_for_prune(ttl_days, namespace)?;
        let ids: Vec<String> = deprecated.iter().map(|m| m.id.clone()).collect();
        let count = ids.len();

        if dry_run || count == 0 {
            return Ok(PruneResult {
                pruned: 0,
                ids: vec![],
                deprecated: count,
                deprecated_ids: ids,
            });
        }

        // Delete by specific IDs to avoid TOCTOU race (not re-query by criteria)
        let pruned = self.store.delete_by_ids(&ids)?;

        // Remove from vector index
        {
            let mut index = self
                .index
                .write()
                .map_err(|_| Error::lock("index write lock during prune"))?;
            for id in &ids {
                index.remove(id);
            }
            if let Err(e) = index.save() {
                tracing::warn!("Failed to save index: {e}");
            }
        }

        Ok(PruneResult {
            pruned,
            ids: ids.clone(),
            deprecated: count,
            deprecated_ids: ids,
        })
    }

    /// Graceful shutdown — save dirty index to disk.
    pub fn shutdown(&self) -> Result<(), Error> {
        let mut index = self
            .index
            .write()
            .map_err(|_| Error::lock("index write lock during shutdown"))?;
        if index.is_dirty() {
            index.save()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::memory::types::AgingStatus;

    #[test]
    fn test_aging_status_type_serialization() {
        let status = AgingStatus {
            total: 100,
            hot: 10,
            warm: 20,
            cold: 50,
            never_accessed: 20,
        };
        let json = serde_json::to_string(&status).unwrap();
        let restored: AgingStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.total, 100);
        assert_eq!(restored.never_accessed, 20);
    }

    #[test]
    fn test_prune_result_serialization() {
        use crate::memory::types::PruneResult;
        let result = PruneResult {
            pruned: 5,
            ids: vec!["a".to_string(), "b".to_string()],
            deprecated: 3,
            deprecated_ids: vec!["c".to_string()],
        };
        let json = serde_json::to_string(&result).unwrap();
        let restored: PruneResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.pruned, 5);
        assert_eq!(restored.deprecated, 3);
    }

    #[test]
    fn test_cleanup_result_serialization() {
        use crate::memory::types::CleanupResult;
        let result = CleanupResult { deleted: 5 };
        let json = serde_json::to_string(&result).unwrap();
        let restored: CleanupResult = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.deleted, 5);
    }
}

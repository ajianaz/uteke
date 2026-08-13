//! Agent-facing memory tools guide (#1010).
//!
//! Provides a standardized guide string that can be injected into agent system
//! prompts alongside recalled memories, teaching the agent how to actively
//! retrieve deeper memories when injected context is insufficient.

/// Available memory tools that an agent integration can expose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryTool {
    /// Semantic hybrid search (vector + FTS5, ranked by RRF).
    Recall,
    /// FTS5 keyword-only search.
    Search,
    /// Scoped recall within a specific Room.
    RoomRecall,
    /// Store a new memory.
    Remember,
}

impl MemoryTool {
    /// Returns the tool name as it would appear in an MCP/API interface.
    pub fn name(self) -> &'static str {
        match self {
            Self::Recall => "uteke_recall",
            Self::Search => "uteke_search",
            Self::RoomRecall => "uteke_room_recall",
            Self::Remember => "uteke_remember",
        }
    }

    /// Short description for the guide.
    fn description(self) -> &'static str {
        match self {
            Self::Recall => "Semantic hybrid search across all memory types. Use for most queries.",
            Self::Search => "FTS5 keyword search. Use when you need exact term matches.",
            Self::RoomRecall => "Scoped search within a specific Room namespace.",
            Self::Remember => "Store a new memory (fact, decision, procedure, etc.).",
        }
    }
}

/// Returns a formatted guide for agent-facing memory tool usage.
///
/// Designed to be appended to a system prompt alongside recalled memories.
/// Only documents the tools the integration actually exposes.
///
/// # Arguments
/// * `available_tools` - Slice of tools the agent can call.
/// * `max_searches` - Max recall/search calls recommended per turn.
pub fn recall_guide(available_tools: &[MemoryTool], max_searches: u32) -> String {
    let mut tools_section = String::new();
    for tool in available_tools {
        tools_section.push_str(&format!("- **{}**: {}\n", tool.name(), tool.description()));
    }

    format!(
        r#"<memory-tools-guide>
## Available Memory Tools

{tools_section}## Guidelines

- Injected context above was pre-fetched. If it fully answers the query, **do not search again**.
- If context is insufficient, use `{recall_tool}` for deeper retrieval.
- **Max {max} search call(s) per turn.** If {max} search(es) yield nothing relevant, the information is likely not stored — answer with available context.
- Use `{remember_tool}` only for genuinely new facts, decisions, or preferences — not for re-stating what's already stored.
</memory-tools-guide>"#,
        tools_section = tools_section,
        recall_tool = available_tools
            .iter()
            .find(|t| **t == MemoryTool::Recall)
            .map(|t| t.name())
            .unwrap_or("uteke_recall"),
        remember_tool = available_tools
            .iter()
            .find(|t| **t == MemoryTool::Remember)
            .map(|t| t.name())
            .unwrap_or("uteke_remember"),
        max = max_searches,
    )
}

/// Convenience: the default guide with all four tools and max 3 searches.
pub fn default_guide() -> String {
    recall_guide(
        &[
            MemoryTool::Recall,
            MemoryTool::Search,
            MemoryTool::RoomRecall,
            MemoryTool::Remember,
        ],
        3,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_guide_contains_all_tools() {
        let guide = default_guide();
        assert!(guide.contains("uteke_recall"));
        assert!(guide.contains("uteke_search"));
        assert!(guide.contains("uteke_room_recall"));
        assert!(guide.contains("uteke_remember"));
    }

    #[test]
    fn test_default_guide_has_max_searches() {
        let guide = default_guide();
        assert!(guide.contains("Max 3 search"));
    }

    #[test]
    fn test_custom_tools_subset() {
        let guide = recall_guide(&[MemoryTool::Recall], 2);
        assert!(guide.contains("uteke_recall"));
        assert!(!guide.contains("uteke_search"));
        assert!(guide.contains("Max 2 search"));
    }

    #[test]
    fn test_guide_is_wrapped_in_tags() {
        let guide = default_guide();
        assert!(guide.starts_with("<memory-tools-guide>"));
        assert!(guide.ends_with("</memory-tools-guide>"));
    }
}

//! Contradiction ledger subcommands — list, undo (#1172 Fase 2).

use crate::cli::Cli;
use crate::cli::ContradictionCommands;
use crate::output;
use uteke_core::Uteke;

pub(crate) fn run(cli: &Cli, uteke: &Uteke, command: &ContradictionCommands) -> Result<(), String> {
    match command {
        ContradictionCommands::List { namespace, limit } => {
            tracing::info!(
                "Listing contradiction resolutions (namespace={namespace:?}, limit={limit})"
            );
            let resolutions = uteke
                .contradiction_resolutions(namespace.as_deref(), *limit)
                .map_err(|e| format!("Failed to list contradiction resolutions: {e}"))?;
            if cli.json {
                output::print_json(&resolutions);
            } else if resolutions.is_empty() {
                println!("No contradiction resolutions found.");
            } else {
                println!("Contradiction resolutions ({} total):\n", resolutions.len());
                for r in &resolutions {
                    let deprecated_at = r
                        .deprecated_at
                        .as_ref()
                        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "unknown".into());
                    println!("  {}  [{}]", short_id(&r.id), deprecated_at);
                    let preview: String = r.content.chars().take(60).collect();
                    println!("    {}", preview);
                    println!(
                        "    reason: {}",
                        r.deprecate_reason.as_deref().unwrap_or("—")
                    );
                }
            }
            Ok(())
        }
        ContradictionCommands::Undo { id } => {
            tracing::info!("Undoing supersession for memory {id}");
            // Accept full UUID or unambiguous prefix (same contract as MCP).
            let resolved = if id.len() == 36 {
                id.clone()
            } else {
                match uteke
                    .resolve_id_prefix(id)
                    .map_err(|e| format!("Failed to resolve id: {e}"))?
                {
                    Some(full) => full,
                    None => return Err(format!("No memory matches id prefix '{id}'")),
                }
            };
            match uteke
                .undo_supersession(&resolved)
                .map_err(|e| format!("Failed to undo supersession: {e}"))?
            {
                Some(winner) => {
                    if cli.json {
                        output::print_json(&serde_json::json!({
                            "undone": true,
                            "restored": resolved,
                            "was_superseded_by": winner,
                        }));
                    } else {
                        println!("✓ Restored memory {resolved}");
                        println!("  was superseded by {winner}");
                        println!("  supersession edges removed — the pair is no longer flagged");
                    }
                }
                None => {
                    return Err(format!("No supersession found for memory: {id}"));
                }
            }
            Ok(())
        }
    }
}

/// Short ID helper for the human-readable ledger listing.
fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

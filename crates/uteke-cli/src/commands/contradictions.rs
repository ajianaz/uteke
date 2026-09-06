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

/// `uteke supersede old new [--reason text]` (#1053) — CLI parity with the
/// MCP tool and HTTP surface. Accepts full UUIDs or unambiguous prefixes.
pub(crate) fn supersede(
    cli: &Cli,
    uteke: &Uteke,
    old: &str,
    new: &str,
    reason: Option<&str>,
) -> Result<(), String> {
    tracing::info!("Superseding {old} -> {new}");
    let resolve = |id: &str| -> Result<String, String> {
        if id.len() == 36 {
            return Ok(id.to_string());
        }
        match uteke
            .resolve_id_prefix(id)
            .map_err(|e| format!("Failed to resolve id: {e}"))?
        {
            Some(full) => Ok(full),
            None => Err(format!("No memory matches id prefix '{id}'")),
        }
    };
    let old_id = resolve(old)?;
    let new_id = resolve(new)?;

    let (o, n) = uteke
        .supersede(&old_id, &new_id, reason)
        .map_err(|e| format!("Failed to supersede: {e}"))?;
    if cli.json {
        output::print_json(&serde_json::json!({
            "superseded": o,
            "by": n,
            "reason": reason,
        }));
    } else {
        println!("✓ Superseded {} → {}", short_id(&o), short_id(&n));
        if let Some(r) = reason {
            println!("  reason: {r}");
        }
        println!(
            "  recall now flags the pair; restore: uteke contradictions undo {}",
            short_id(&o)
        );
    }
    Ok(())
}

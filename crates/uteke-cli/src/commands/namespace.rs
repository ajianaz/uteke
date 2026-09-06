//! Namespace subcommands — list, stats, switch.

use crate::Config;
use crate::cli::Cli;
use crate::cli::NamespaceCommands;
use crate::output;
use uteke_core::Uteke;

pub(crate) fn run(cli: &Cli, uteke: &Uteke, command: &NamespaceCommands) -> Result<(), String> {
    match command {
        NamespaceCommands::List => {
            tracing::info!("Listing namespaces");
            let namespaces = uteke
                .list_namespaces()
                .map_err(|e| format!("Failed to list namespaces: {e}"))?;
            if cli.json {
                output::print_json(&namespaces);
            } else if namespaces.is_empty() {
                println!("No namespaces found.");
            } else {
                println!("Namespaces ({} total):\n", namespaces.len());
                for ns_name in &namespaces {
                    let count = uteke.count(Some(ns_name.as_str())).unwrap_or(0);
                    println!("  {} ({} memories)", ns_name, count);
                }
            }
            Ok(())
        }
        NamespaceCommands::Stats { name } => {
            tracing::info!("Namespace stats: {name}");
            let stats = uteke
                .stats(Some(name.as_str()))
                .map_err(|e| format!("Failed to get namespace stats: {e}"))?;
            if cli.json {
                output::print_json(&stats);
            } else {
                println!("Namespace: {name}");
                output::print_stats_human(&stats);
            }
            Ok(())
        }
        NamespaceCommands::Switch { name } => {
            tracing::info!("Switching default namespace to: {name}");
            Config::set_default_namespace(name)
                .map_err(|e| format!("Failed to switch namespace: {e}"))?;
            if cli.json {
                output::print_json(&serde_json::json!({"default_namespace": name}));
            } else {
                println!("\u{2713} Default namespace set to '{name}'");
            }
            Ok(())
        }
        NamespaceCommands::Move { id, namespace } => {
            tracing::info!("Moving memory {id} to namespace '{namespace}'");
            let moved = uteke
                .move_memory(id, namespace)
                .map_err(|e| format!("Failed to move memory: {e}"))?;
            if !moved {
                return Err(format!("Memory not found: {id}"));
            }
            if cli.json {
                output::print_json(&serde_json::json!({
                    "moved": true,
                    "id": id,
                    "namespace": namespace,
                }));
            } else {
                println!("\u{2713} Moved memory {id} to namespace '{namespace}'");
            }
            Ok(())
        }
        NamespaceCommands::Rename { from, to } => {
            tracing::info!("Renaming namespace '{from}' to '{to}'");
            let result = uteke
                .rename_namespace(from, to)
                .map_err(|e| format!("Failed to rename namespace: {e}"))?;
            if cli.json {
                output::print_json(&result);
            } else {
                let kind = if result.target_existed {
                    "merged into existing"
                } else {
                    "renamed to"
                };
                println!(
                    "\u{2713} Namespace '{from}' {kind} '{to}' — {} memories moved",
                    result.moved
                );
            }
            Ok(())
        }
        NamespaceCommands::Delete {
            name,
            strategy,
            target,
            confirm,
        } => {
            if !confirm {
                return Err(
                    "Refusing to delete a namespace without --confirm (this affects its memories)"
                        .to_string(),
                );
            }
            tracing::info!("Deleting namespace '{name}' (strategy={strategy})");
            let result = uteke
                .delete_namespace(name, strategy, target.as_deref())
                .map_err(|e| format!("Failed to delete namespace: {e}"))?;
            if cli.json {
                output::print_json(&result);
            } else {
                match result.strategy.as_str() {
                    "merge" => println!(
                        "\u{2713} Moved {} memories from '{}' to '{}' — namespace removed",
                        result.affected,
                        result.name,
                        result.target.as_deref().unwrap_or("?")
                    ),
                    "deprecate" => println!(
                        "\u{2713} Soft-deleted {} memories in '{}' (restorable via promote; the name stays visible as deprecated-only)",
                        result.affected, result.name
                    ),
                    _ => println!("\u{2713} Namespace '{}' deleted", result.name),
                }
            }
            Ok(())
        }
    }
}

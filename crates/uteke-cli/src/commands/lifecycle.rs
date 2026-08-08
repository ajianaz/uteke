//! Lifecycle CLI commands (#935): cycle, promote, status.

use crate::cli::LifecycleCommands;
use uteke_core::Uteke;

/// Run lifecycle subcommands.
pub(crate) fn run(
    uteke: &Uteke,
    ns: Option<&str>,
    command: &LifecycleCommands,
) -> Result<(), String> {
    match command {
        LifecycleCommands::Cycle { namespace } => {
            let target_ns = namespace.as_deref().or(ns);
            let result = uteke
                .lifecycle_cycle(target_ns)
                .map_err(|e| format!("{e}"))?;

            println!("Lifecycle cycle complete:");
            println!("  Active memories:      {}", result.total_active);
            println!("  Aged candidates:      {}", result.candidates);
            println!(
                "  Cap applied:          {} (clamped to min/max)",
                result.cap
            );
            println!("  Deprecated this run:  {}", result.deprecated);
            if !result.deprecated_ids.is_empty() {
                println!("    IDs: {}", result.deprecated_ids.join(", "));
            }
            println!("  Pruned (hard delete): {}", result.pruned);
            if !result.pruned_ids.is_empty() {
                println!("    IDs: {}", result.pruned_ids.join(", "));
            }

            Ok(())
        }

        LifecycleCommands::Promote { id } => {
            uteke.promote(id).map_err(|e| format!("{e}"))?;
            println!("✓ Memory {id} promoted back to active status.");
            Ok(())
        }

        LifecycleCommands::Status { namespace } => {
            let target_ns = namespace.as_deref().or(ns);
            let active = uteke
                .store()
                .count_active(target_ns)
                .map_err(|e| format!("{e}"))?;
            let deprecated = uteke
                .store()
                .count_deprecated(target_ns)
                .map_err(|e| format!("{e}"))?;
            let cfg = uteke.get_lifecycle_config();

            println!(
                "Lifecycle Status (namespace: {})",
                target_ns.unwrap_or("all")
            );
            println!("  Active memories:     {active}");
            println!("  Deprecated memories: {deprecated}");
            println!();
            println!("Configuration:");
            println!("  soft_delete_only:          {}", cfg.soft_delete_only);
            println!("  auto_aging_enabled:        {}", cfg.auto_aging_enabled);
            println!(
                "  auto_aging_interval_hours: {}",
                cfg.auto_aging_interval_hours
            );
            println!("  min_age_days:              {}", cfg.min_age_days);
            println!("  max_access_count:          {}", cfg.max_access_count);
            println!(
                "  max_deprecate_percent:     {}%",
                cfg.max_deprecate_percent
            );
            println!("  deprecated_ttl_days:       {}", cfg.deprecated_ttl_days);
            println!("  auto_prune_enabled:        {}", cfg.auto_prune_enabled);

            Ok(())
        }
    }
}

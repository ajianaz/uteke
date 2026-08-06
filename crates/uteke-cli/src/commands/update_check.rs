//! Background startup update notification.
//!
//! Checks GitHub for a newer release at most once per 24h.
//! Prints a banner to stderr if an update is available.
//! Never auto-upgrades — notification only.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::Config;

/// Cache file path: `~/.config/uteke/update-cache.json` (or platform equivalent).
fn cache_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("uteke").join("update-cache.json"))
}

/// Cache payload persisted between runs.
#[derive(serde::Serialize, serde::Deserialize)]
struct Cache {
    /// Unix timestamp (seconds) of last check.
    checked_at: u64,
    /// Latest version tag found (e.g. `v0.14.0`).
    latest: String,
}

/// Seconds before re-checking GitHub (24 hours).
const CACHE_TTL: u64 = 86_400;

/// Entry point: returns a `JoinHandle` that the caller should `.join()`
/// at the end of `main()` to ensure the thread isn't killed prematurely.
///
/// - If cache is fresh, prints immediately and returns `None` (no thread).
/// - If cache is stale/missing, spawns a background thread and returns
///   `Some(handle)`.
pub fn spawn() -> Option<std::thread::JoinHandle<()>> {
    // Respect config opt-out.
    if !enabled() {
        return None;
    }

    // Try cache first — if fresh, print immediately and skip network.
    if let Some(latest) = read_cache() {
        let current = env!("CARGO_PKG_VERSION");
        if is_newer(&latest, current) {
            print_banner(&latest, current);
        }
        return None;
    }

    // Cache stale or missing — spawn background check.
    // Caller MUST join() this handle before process exit so the thread
    // isn't killed before the network request completes.
    let handle = std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(|| {
            run_check();
        });
    });

    Some(handle)
}

/// Read config to determine if update check is enabled.
/// Defaults to `true` when config field is unset.
fn enabled() -> bool {
    Config::load().update_check
}

/// Background thread body: fetch latest version, update cache, print banner.
fn run_check() {
    let latest = match crate::commands::upgrade::get_latest_version() {
        Ok(tag) => tag,
        Err(_) => return, // network failure — silently skip
    };

    write_cache(&latest);

    let current = env!("CARGO_PKG_VERSION");
    if is_newer(&latest, current) {
        print_banner(&latest, current);
    }
}

/// Read cache. Returns `Some(latest_version)` if cache is fresh (< 24h).
fn read_cache() -> Option<String> {
    let path = cache_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    let cache: Cache = serde_json::from_str(&data).ok()?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if now.saturating_sub(cache.checked_at) < CACHE_TTL {
        Some(cache.latest)
    } else {
        None
    }
}

/// Persist cache to disk. Best-effort — ignores errors.
fn write_cache(latest: &str) {
    let Some(path) = cache_path() else { return };

    // Ensure parent dir exists.
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let cache = Cache {
        checked_at: now,
        latest: latest.to_string(),
    };

    if let Ok(json) = serde_json::to_string(&cache) {
        let _ = std::fs::write(&path, json);
    }
}

/// Compare semver-ish versions. Returns true if `latest` > `current`.
/// Strips leading `v` prefix. Falls back to string comparison on parse failure.
fn is_newer(latest: &str, current: &str) -> bool {
    let lt = latest.trim_start_matches('v');
    let cur = current.trim_start_matches('v');

    let parse = |s: &str| -> Vec<u64> {
        s.split('.')
            .filter_map(|p| p.split('-').next().and_then(|n| n.parse().ok()))
            .collect()
    };

    match (parse(lt), parse(cur)) {
        (lt_parts, cur_parts) if !lt_parts.is_empty() && !cur_parts.is_empty() => {
            lt_parts > cur_parts
        }
        _ => lt > cur,
    }
}

/// Print update banner to stderr.
fn print_banner(latest: &str, current: &str) {
    eprintln!(
        "\n⚠ Update available: {latest}\n  \
         Run `uteke upgrade` to update (currently v{current})\n  \
         https://github.com/codecoradev/uteke/releases/tag/{latest}\n"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_true() {
        assert!(is_newer("v0.14.0", "0.13.0"));
        assert!(is_newer("0.14.0", "0.13.0"));
        assert!(is_newer("v1.0.0", "0.99.99"));
    }

    #[test]
    fn test_is_newer_false() {
        assert!(!is_newer("v0.13.0", "0.13.0"));
        assert!(!is_newer("v0.12.0", "0.13.0"));
    }

    #[test]
    fn test_is_newer_major() {
        assert!(is_newer("v2.0.0", "1.9.9"));
        assert!(!is_newer("v1.9.9", "2.0.0"));
    }
}

//! Background startup update notification — thin wrapper over `uteke_core::update_check`.
//!
//! Delegates all logic (fetch, cache, compare, banner) to core so MCP and HTTP
//! servers share the same code path. The config opt-out (`update_check = false`
//! in `uteke.toml`) is CLI-specific — core doesn't know about user config.

/// Entry point: returns a `JoinHandle` that the caller should `.join()`
/// at the end of `main()` to ensure the thread isn't killed prematurely.
///
/// - If cache is fresh, prints immediately and returns `None` (no thread).
/// - If cache is stale/missing, spawns a background thread and returns
///   `Some(handle)`.
/// - Respects `update_check = false` from `uteke.toml` (CLI-only).
pub fn spawn() -> Option<std::thread::JoinHandle<()>> {
    // Try cache first — if fresh, print immediately and skip network.
    if let Some(info) = uteke_core::update_check::check_cached() {
        if info.is_update_available() {
            eprintln!("\n{}\n", info.banner());
        }
        return None;
    }

    // Cache stale or missing — spawn background check.
    // Config opt-out check happens inside the thread to avoid
    // synchronous disk I/O on the main thread.
    let handle = std::thread::spawn(move || {
        let _ = std::panic::catch_unwind(|| {
            // Respect config opt-out (checked in background thread).
            if !enabled() {
                return;
            }
            if let Ok(info) = uteke_core::update_check::check_network() {
                if info.is_update_available() {
                    eprintln!("\n{}\n", info.banner());
                }
            }
        });
    });

    Some(handle)
}

/// Read config to determine if update check is enabled.
/// Defaults to `true` when config field is unset.
fn enabled() -> bool {
    crate::Config::load().update_check
}

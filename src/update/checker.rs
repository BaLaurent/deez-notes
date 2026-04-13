use std::path::PathBuf;
use std::sync::mpsc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use semver::Version;

use super::{InstallMethod, UpdateStatus};

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    latest_version: String,
    checked_at: DateTime<Utc>,
}

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("deez-notes").join("update-check.json"))
}

fn read_cache() -> Option<CacheEntry> {
    let path = cache_path()?;
    let data = std::fs::read_to_string(&path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_cache(entry: &CacheEntry) {
    if let Some(path) = cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(json) = serde_json::to_string(entry) {
            let _ = std::fs::write(&path, json);
        }
    }
}

fn is_cache_fresh(entry: &CacheEntry) -> bool {
    let age = Utc::now() - entry.checked_at;
    age < chrono::Duration::hours(24)
}

// ---------------------------------------------------------------------------
// Installation method detection
// ---------------------------------------------------------------------------

fn detect_install_method() -> InstallMethod {
    let exe = std::env::current_exe().ok();

    if let Some(ref path) = exe {
        // Cargo install: binary lives in ~/.cargo/bin/
        if let Some(home) = dirs::home_dir() {
            let cargo_bin = home.join(".cargo").join("bin");
            if path.starts_with(&cargo_bin) {
                return InstallMethod::Cargo;
            }
        }

        // Pacman-managed: binary in /usr/bin/ and owned by pacman
        let path_str = path.to_string_lossy();
        if path_str.starts_with("/usr/bin/") || path_str.starts_with("/usr/local/bin/") {
            if std::process::Command::new("pacman")
                .args(["-Qo", &path_str])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                return InstallMethod::Pacman;
            }
        }
    }

    InstallMethod::ManualBinary
}

// ---------------------------------------------------------------------------
// Version comparison
// ---------------------------------------------------------------------------

fn compare_versions(current: &str, latest: &str, method: InstallMethod) -> UpdateStatus {
    let current_ver = match Version::parse(current) {
        Ok(v) => v,
        Err(_) => return UpdateStatus::Failed(format!("invalid current version: {current}")),
    };
    let latest_ver = match Version::parse(latest) {
        Ok(v) => v,
        Err(_) => return UpdateStatus::Failed(format!("invalid latest version: {latest}")),
    };

    if latest_ver > current_ver {
        UpdateStatus::Available {
            current: current.to_string(),
            latest: latest.to_string(),
            install_method: method,
        }
    } else {
        UpdateStatus::UpToDate
    }
}

// ---------------------------------------------------------------------------
// Background check
// ---------------------------------------------------------------------------

fn check_update(current_version: &str) -> UpdateStatus {
    let install_method = detect_install_method();

    // Try cache first.
    if let Some(cache) = read_cache() {
        if is_cache_fresh(&cache) {
            return compare_versions(current_version, &cache.latest_version, install_method);
        }
    }

    // Cache stale or missing — fetch from GitHub.
    match super::github::fetch_latest_version() {
        Ok(latest) => {
            write_cache(&CacheEntry {
                latest_version: latest.clone(),
                checked_at: Utc::now(),
            });
            compare_versions(current_version, &latest, install_method)
        }
        Err(e) => UpdateStatus::Failed(e),
    }
}

/// Spawn a background thread that checks for updates.
///
/// Returns a `Receiver` that will receive exactly one `UpdateStatus`.
pub fn spawn_check() -> mpsc::Receiver<UpdateStatus> {
    let (tx, rx) = mpsc::channel();
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    std::thread::Builder::new()
        .name("update-check".into())
        .spawn(move || {
            let status = check_update(&current_version);
            let _ = tx.send(status);
        })
        .expect("failed to spawn update check thread");

    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_detected() {
        let status = compare_versions("0.3.0", "0.4.0", InstallMethod::Cargo);
        assert!(matches!(status, UpdateStatus::Available { .. }));
        if let UpdateStatus::Available { current, latest, install_method } = status {
            assert_eq!(current, "0.3.0");
            assert_eq!(latest, "0.4.0");
            assert_eq!(install_method, InstallMethod::Cargo);
        }
    }

    #[test]
    fn same_version_is_up_to_date() {
        let status = compare_versions("0.3.0", "0.3.0", InstallMethod::ManualBinary);
        assert!(matches!(status, UpdateStatus::UpToDate));
    }

    #[test]
    fn older_remote_is_up_to_date() {
        let status = compare_versions("0.4.0", "0.3.0", InstallMethod::Pacman);
        assert!(matches!(status, UpdateStatus::UpToDate));
    }

    #[test]
    fn semver_comparison_handles_multi_digit() {
        let status = compare_versions("0.9.0", "0.10.0", InstallMethod::Cargo);
        assert!(matches!(status, UpdateStatus::Available { .. }));
    }

    #[test]
    fn invalid_version_returns_failed() {
        let status = compare_versions("not-a-version", "0.4.0", InstallMethod::Cargo);
        assert!(matches!(status, UpdateStatus::Failed(_)));
    }

    #[test]
    fn cache_freshness() {
        let fresh = CacheEntry {
            latest_version: "0.4.0".to_string(),
            checked_at: Utc::now(),
        };
        assert!(is_cache_fresh(&fresh));

        let stale = CacheEntry {
            latest_version: "0.4.0".to_string(),
            checked_at: Utc::now() - chrono::Duration::hours(25),
        };
        assert!(!is_cache_fresh(&stale));
    }

    #[test]
    fn cache_round_trip() {
        let entry = CacheEntry {
            latest_version: "0.5.0".to_string(),
            checked_at: Utc::now(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let parsed: CacheEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.latest_version, "0.5.0");
    }

    #[test]
    fn spawn_check_returns_receiver() {
        let rx = spawn_check();
        // The thread will likely fail (no network in tests) or succeed if cached.
        // We verify the channel delivers a value within a reasonable timeout.
        let status = rx.recv_timeout(std::time::Duration::from_secs(15)).unwrap();
        // Any variant is acceptable — the test validates the thread/channel mechanism.
        match status {
            UpdateStatus::Available { .. }
            | UpdateStatus::UpToDate
            | UpdateStatus::Failed(_) => {}
        }
    }
}

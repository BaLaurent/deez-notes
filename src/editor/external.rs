use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Fallback editors tried in order when neither `editor_override` nor `$EDITOR`
/// is available.
const FALLBACK_EDITORS: &[&str] = &["micro", "nano", "vi"];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Open `path` in an external editor, blocking until the editor exits.
///
/// Editor resolution order:
/// 1. `editor_override` (if provided)
/// 2. `$EDITOR` environment variable
/// 3. Fallback chain: micro -> nano -> vi
///
/// The editor is spawned via `std::process::Command` with `path` passed as a
/// separate argument (no shell interpolation) to prevent command injection.
pub fn open_in_editor(path: &Path, editor_override: Option<&str>) -> anyhow::Result<()> {
    let editor = find_editor(editor_override)?;

    let status = Command::new(&editor)
        .arg(path)
        .status()
        .with_context(|| format!(
            "failed to launch editor '{}'. Is it installed? Try setting $EDITOR to your preferred editor",
            editor
        ))?;

    if status.success() {
        return Ok(());
    }

    match status.code() {
        Some(code) => bail!("Editor '{}' exited with status {}", editor, code),
        None => bail!("Editor '{}' was terminated by a signal", editor),
    }
}

/// Resolve which editor binary would be used, without spawning it.
///
/// Same resolution order as `open_in_editor`. Useful for checking at startup
/// whether a valid editor is available.
pub fn find_editor(editor_override: Option<&str>) -> anyhow::Result<String> {
    if let Some(name) = editor_override {
        if !name.is_empty() {
            return Ok(name.to_string());
        }
    }

    if let Ok(env_editor) = std::env::var("EDITOR") {
        if !env_editor.is_empty() {
            return Ok(env_editor);
        }
    }

    for candidate in FALLBACK_EDITORS {
        if command_exists(candidate) {
            return Ok((*candidate).to_string());
        }
    }

    bail!(
        "No editor found. Set $EDITOR or install one of: {}",
        FALLBACK_EDITORS.join(", ")
    )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether an executable exists on `$PATH`.
pub(crate) fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(name).is_file())
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Viewer (read-only)
// ---------------------------------------------------------------------------

/// Fallback viewers tried in order: mcat (pretty Markdown viewer) then cat.
const FALLBACK_VIEWERS: &[&str] = &["mcat", "cat"];

/// Resolve which viewer binary would be used.
///
/// Resolution order: mcat -> cat (first available on `$PATH`).
pub fn find_viewer() -> anyhow::Result<String> {
    for candidate in FALLBACK_VIEWERS {
        if command_exists(candidate) {
            return Ok((*candidate).to_string());
        }
    }

    bail!(
        "No viewer found. Install one of: {}",
        FALLBACK_VIEWERS.join(", ")
    )
}

/// Viewers that are interactive pagers (handle scrolling themselves).
const PAGER_VIEWERS: &[&str] = &["mcat", "less", "more", "bat", "most"];

/// Returns `true` if the given viewer name is an interactive pager that handles
/// its own input (scrolling, quitting, etc.).
pub fn viewer_is_pager(viewer: &str) -> bool {
    PAGER_VIEWERS.contains(&viewer)
}

/// Open `path` in a read-only viewer, blocking until the viewer exits.
///
/// Viewer resolution: mcat if available, otherwise cat.
/// Returns the viewer name used so callers can decide whether to wait for a
/// keypress (non-pager viewers like `cat` dump output and exit immediately).
///
/// When the viewer is `mcat`, `--paging always` is added so its built-in
/// pager (`less -r`) stays open instead of dumping output and exiting.
pub fn open_in_viewer(path: &Path) -> anyhow::Result<String> {
    let viewer = find_viewer()?;

    let mut cmd = Command::new(&viewer);
    // mcat in "auto" paging mode may skip the pager when launched from a TUI
    // app, so force it.
    if viewer == "mcat" {
        cmd.arg("--paging").arg("always");
    }
    let status = cmd
        .arg(path)
        .status()
        .with_context(|| format!(
            "failed to launch viewer '{}'. Is it installed?",
            viewer
        ))?;

    if status.success() {
        return Ok(viewer);
    }

    match status.code() {
        Some(code) => bail!("Viewer '{}' exited with status {}", viewer, code),
        None => bail!("Viewer '{}' was terminated by a signal", viewer),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Tests that mutate `$EDITOR` must run serially to avoid race conditions.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn find_editor_uses_override_first() {
        // Override wins regardless of env — no env mutation needed.
        let result = find_editor(Some("my-custom-editor"));
        assert_eq!(result.expect("should succeed"), "my-custom-editor");
    }

    #[test]
    fn find_editor_ignores_empty_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("EDITOR").ok();
        // SAFETY: test holds ENV_LOCK so no concurrent env access.
        unsafe { std::env::set_var("EDITOR", "test-editor-from-env"); }

        let result = find_editor(Some(""));
        assert_eq!(result.expect("should succeed"), "test-editor-from-env");

        match prev {
            Some(v) => unsafe { std::env::set_var("EDITOR", v); },
            None => unsafe { std::env::remove_var("EDITOR"); },
        }
    }

    #[test]
    fn find_editor_reads_env_when_no_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("EDITOR").ok();
        // SAFETY: test holds ENV_LOCK so no concurrent env access.
        unsafe { std::env::set_var("EDITOR", "helix"); }

        let result = find_editor(None);
        assert_eq!(result.expect("should succeed"), "helix");

        match prev {
            Some(v) => unsafe { std::env::set_var("EDITOR", v); },
            None => unsafe { std::env::remove_var("EDITOR"); },
        }
    }

    #[test]
    fn find_editor_fallback_when_no_env() {
        let _guard = ENV_LOCK.lock().unwrap();
        let prev = std::env::var("EDITOR").ok();
        // SAFETY: test holds ENV_LOCK so no concurrent env access.
        unsafe { std::env::remove_var("EDITOR"); }

        let result = find_editor(None);
        let editor = result.expect("should find at least one fallback editor");

        assert!(
            FALLBACK_EDITORS.contains(&editor.as_str()),
            "expected a fallback editor, got: {}",
            editor
        );

        if let Some(v) = prev {
            unsafe { std::env::set_var("EDITOR", v); }
        }
    }

    #[test]
    fn find_viewer_returns_available_viewer() {
        let result = find_viewer();
        let viewer = result.expect("should find at least cat");
        assert!(
            FALLBACK_VIEWERS.contains(&viewer.as_str()),
            "expected a fallback viewer, got: {}",
            viewer
        );
    }

    #[test]
    fn find_viewer_prefers_mcat_over_cat() {
        let viewer = find_viewer().unwrap();
        if command_exists("mcat") {
            assert_eq!(viewer, "mcat");
        } else {
            assert_eq!(viewer, "cat");
        }
    }

    #[test]
    fn command_exists_finds_common_binary() {
        // "sh" should exist on any Unix system
        assert!(command_exists("sh"));
    }

    #[test]
    fn command_exists_returns_false_for_nonsense() {
        assert!(!command_exists("definitely-not-a-real-binary-xyz-12345"));
    }
}

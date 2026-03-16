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

/// Open `path` in a read-only viewer, blocking until the viewer exits.
///
/// If `pager` is provided, it is used with `pager_args` prepended before the
/// file path.  Otherwise falls back to `cat` with no extra arguments.
///
/// Returns `true` when the viewer is a configured pager (interactive — handles
/// scrolling itself), `false` when it is the `cat` fallback (dumps output and
/// exits immediately, so the caller should wait for a keypress).
pub fn open_in_viewer(
    path: &Path,
    pager: Option<&str>,
    pager_args: &[String],
) -> anyhow::Result<bool> {
    let (viewer, is_pager) = match pager {
        Some(name) if !name.is_empty() => (name.to_string(), true),
        _ => ("cat".to_string(), false),
    };

    let mut cmd = Command::new(&viewer);
    if is_pager {
        for arg in pager_args {
            cmd.arg(arg);
        }
    }
    let status = cmd
        .arg(path)
        .status()
        .with_context(|| format!(
            "failed to launch viewer '{}'. Is it installed?",
            viewer
        ))?;

    if status.success() {
        return Ok(is_pager);
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
    fn open_in_viewer_uses_configured_pager() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file = dir.path().join("test.md");
        std::fs::write(&file, "# Hello").expect("write file");

        // `cat` is available everywhere — use it as the "configured pager".
        let args = vec![];
        let is_pager = open_in_viewer(&file, Some("cat"), &args)
            .expect("cat should succeed");
        assert!(is_pager, "configured pager should report is_pager = true");
    }

    #[test]
    fn open_in_viewer_falls_back_to_cat() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file = dir.path().join("test.md");
        std::fs::write(&file, "# Hello").expect("write file");

        let is_pager = open_in_viewer(&file, None, &[])
            .expect("cat fallback should succeed");
        assert!(!is_pager, "cat fallback should report is_pager = false");
    }

    #[test]
    fn open_in_viewer_ignores_empty_pager() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let file = dir.path().join("test.md");
        std::fs::write(&file, "# Hello").expect("write file");

        let is_pager = open_in_viewer(&file, Some(""), &[])
            .expect("empty pager should fall back to cat");
        assert!(!is_pager);
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

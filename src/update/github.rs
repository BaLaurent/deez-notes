use std::time::Duration;

use serde::Deserialize;

/// Minimal subset of the GitHub releases API response.
#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

/// Fetch the latest release version tag from GitHub.
///
/// Returns the version string with any leading `v` stripped.
/// This function performs a blocking HTTP call — run it on a background thread.
pub fn fetch_latest_version() -> Result<String, String> {
    let url = "https://api.github.com/repos/BaLaurent/deez-notes/releases/latest";

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(10))
        .build();

    let response = agent
        .get(url)
        .set(
            "User-Agent",
            &format!("deez-notes/{}", env!("CARGO_PKG_VERSION")),
        )
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .map_err(|e| format!("HTTP request failed: {e}"))?;

    let release: GithubRelease = response
        .into_json()
        .map_err(|e| format!("Failed to parse response: {e}"))?;

    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name);
    Ok(version.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_github_release() {
        let json = r#"{"tag_name": "v0.4.0", "name": "Release 0.4.0"}"#;
        let release: GithubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v0.4.0");
    }

    #[test]
    fn strip_v_prefix() {
        let tag = "v1.2.3";
        let version = tag.strip_prefix('v').unwrap_or(tag);
        assert_eq!(version, "1.2.3");
    }

    #[test]
    fn no_v_prefix_unchanged() {
        let tag = "1.2.3";
        let version = tag.strip_prefix('v').unwrap_or(tag);
        assert_eq!(version, "1.2.3");
    }
}

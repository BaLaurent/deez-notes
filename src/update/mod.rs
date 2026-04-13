pub mod checker;
pub mod github;

/// How the user installed deez-notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    Cargo,
    Pacman,
    ManualBinary,
}

/// Result of the background update check.
#[derive(Debug, Clone)]
pub enum UpdateStatus {
    /// A newer version exists.
    Available {
        current: String,
        latest: String,
        install_method: InstallMethod,
    },
    /// Already on the latest (or newer) version.
    UpToDate,
    /// Check failed (network error, parse error, etc.). Silently ignored in UI.
    Failed(String),
}

impl UpdateStatus {
    /// The update command or URL appropriate for the installation method.
    pub fn update_hint(&self) -> Option<&'static str> {
        match self {
            Self::Available { install_method, .. } => Some(match install_method {
                InstallMethod::Cargo => "cargo install deez-notes",
                InstallMethod::Pacman => "yay -S deez-notes-bin",
                InstallMethod::ManualBinary => {
                    "https://github.com/BaLaurent/deez-notes/releases"
                }
            }),
            _ => None,
        }
    }
}

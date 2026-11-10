use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const PINNED_DWPROTON_VERSION: &str = "11.0-3";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Extra arguments passed to the game's executable.
    pub launch_options: String,
    /// Installation directory containing the game files.
    pub game_path: Option<String>,
    /// Optional wallpaper override. Empty means use assets/background.*.
    pub wallpaper: Option<String>,
    /// Optional selected Wine/Proton executable or compatibility-tool directory.
    pub wine_runtime: Option<String>,
    /// Opt-in experiment; never changes the KDE color scheme.
    pub hoyoplay_backgrounds: bool,
    /// When false, the API still supplies the static background image.
    pub animated_background: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            launch_options: String::new(),
            game_path: None,
            wallpaper: None,
            wine_runtime: None,
            hoyoplay_backgrounds: false,
            animated_background: true,
        }
    }
}

impl Settings {
    pub fn load(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(path)?;
        Ok(toml::from_str(&content)?)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        let temporary = path.with_extension("toml.tmp");
        std::fs::write(&temporary, content)?;
        std::fs::rename(temporary, path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_file_uses_defaults() {
        let temp = tempfile::tempdir().unwrap();
        let settings = Settings::load(&temp.path().join("settings.toml")).unwrap();
        assert!(settings.launch_options.is_empty());
        assert!(settings.game_path.is_none());
    }

    #[test]
    fn settings_round_trip() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.toml");
        let settings = Settings {
            launch_options: "-windowed -screen-fullscreen 0".into(),
            game_path: Some("/games/example".into()),
            wallpaper: None,
            wine_runtime: Some("/opt/dwproton/proton".into()),
            hoyoplay_backgrounds: true,
            animated_background: false,
        };
        settings.save(&path).unwrap();
        let loaded = Settings::load(&path).unwrap();
        assert_eq!(loaded.launch_options, settings.launch_options);
        assert_eq!(loaded.game_path, settings.game_path);
        assert_eq!(loaded.wine_runtime, settings.wine_runtime);
        assert!(loaded.hoyoplay_backgrounds);
        assert!(!loaded.animated_background);
    }

    #[test]
    fn malformed_settings_are_reported_and_preserved() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("settings.toml");
        let content = "launch_options = [broken";
        std::fs::write(&path, content).unwrap();
        assert!(Settings::load(&path).is_err());
        assert_eq!(std::fs::read_to_string(path).unwrap(), content);
    }
}

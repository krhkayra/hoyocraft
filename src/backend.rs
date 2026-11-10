use crate::{paths::AppPaths, runtime, settings::Settings};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use url::Url;

/// Convert native paths or local dialog URLs exactly once (spaces, #, %, Unicode).
pub fn local_path(value: &str) -> Result<Option<PathBuf>> {
    if value.is_empty() {
        return Ok(None);
    }
    let path = if value.starts_with("file:") {
        Url::parse(value)?
            .to_file_path()
            .map_err(|_| anyhow::anyhow!("Choose a local file or directory"))?
    } else {
        PathBuf::from(value)
    };
    if !path.is_absolute() {
        bail!("Use an absolute local path");
    }
    Ok(Some(path))
}

fn file_url(path: &Path) -> String {
    Url::from_file_path(path)
        .map(|u| u.to_string())
        .unwrap_or_default()
}

pub struct Backend {
    pub paths: AppPaths,
    pub settings: Settings,
}

impl Backend {
    pub fn load() -> Result<Self> {
        let paths = AppPaths::discover()?;
        paths.create_layout()?;
        let settings = Settings::load(&paths.settings_file)
            .context("Could not read settings.toml; the existing file was preserved")?;
        Ok(Self { paths, settings })
    }

    pub fn snapshot(&self) -> Value {
        let mut runtimes = runtime::discover(&self.paths);
        let selected = self
            .settings
            .wine_runtime
            .clone()
            .unwrap_or_else(|| runtimes[0].path.clone());
        if !runtimes.iter().any(|r| r.path == selected) {
            runtimes.push(runtime::Runtime {
                label: "Custom runtime".into(),
                available: runtime::entry_point(Path::new(&selected)).is_some(),
                path: selected.clone(),
            });
        }
        let available = runtime::entry_point(Path::new(&selected)).is_some();
        let wallpaper = self
            .settings
            .wallpaper
            .as_deref()
            .map(PathBuf::from)
            .filter(|p| p.is_file())
            .or_else(|| self.paths.find_default_wallpaper());
        let warning = match self.settings.wallpaper.as_deref() {
            Some(p) if !Path::new(p).is_file() => {
                "Saved wallpaper is missing; using assets/background.* or the desktop palette."
            }
            _ => "",
        };
        json!({
            "settings": self.settings,
            "root": self.paths.root,
            "gameAvailable": crate::game::executable_path(&self.paths, self.settings.game_path.as_deref()).is_some(),
            "gameExecutable": crate::game::installation_directory(&self.paths, self.settings.game_path.as_deref()).join(crate::game::GAME_EXECUTABLE),
            "gamePath": self.settings.game_path.as_deref().unwrap_or_else(|| self.paths.game.to_str().unwrap_or_default()),
            "gameUrl": file_url(Path::new(self.settings.game_path.as_deref().unwrap_or_else(|| self.paths.game.to_str().unwrap_or_default()))),
            "wallpaperUrl": wallpaper.as_deref().map(file_url).unwrap_or_default(),
            "wallpaperWarning": warning,
            "runtimePath": selected,
            "runtimeAvailable": available,
            "runtimeStatus": if available { "Selected runtime is available." } else { "Selected Wine/Proton is missing or not executable. Install it or choose an available runtime in Settings." },
            "runtimes": runtimes,
        })
    }

    pub fn save(
        &mut self,
        options: String,
        game: &str,
        wallpaper: &str,
        wine: &str,
        hoyoplay_backgrounds: bool,
        animated_background: bool,
    ) -> Result<()> {
        let game = local_path(game)?;
        if let Some(path) = &game {
            if !path.is_dir() {
                bail!("Game directory does not exist");
            }
        }
        let wallpaper = local_path(wallpaper)?;
        if let Some(path) = &wallpaper {
            if !path.is_file() {
                bail!("Wallpaper file does not exist");
            }
            let extension = path
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if !["png", "jpg", "jpeg", "webp"].contains(&extension.as_str()) {
                bail!("Choose a PNG, JPEG or WebP wallpaper");
            }
        }
        let wine = local_path(wine)?;
        let pinned = self.paths.wine.join(format!(
            "dwproton-{}",
            crate::settings::PINNED_DWPROTON_VERSION
        ));
        if let Some(path) = &wine {
            // Keep the pinned default configurable before it is installed; reject broken custom entries.
            if path != &pinned && runtime::entry_point(path).is_none() {
                bail!(
                    "Wine/Proton must be an executable or a tool directory containing proton or bin/wine"
                );
            }
        }
        let settings = Settings {
            hoyoplay_backgrounds,
            animated_background,
            launch_options: options,
            game_path: game
                .filter(|p| p != &self.paths.game)
                .map(|p| p.to_string_lossy().into_owned()),
            wallpaper: wallpaper.map(|p| p.to_string_lossy().into_owned()),
            wine_runtime: wine
                .filter(|p| p != &pinned)
                .map(|p| p.to_string_lossy().into_owned()),
        };
        settings
            .save(&self.paths.settings_file)
            .context("Could not save settings")?;
        self.settings = settings;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paths_round_trip_special_characters() {
        let path = Path::new("/tmp/oyun alanı/#100%/背景.png");
        assert_eq!(local_path(&file_url(path)).unwrap().as_deref(), Some(path));
        assert!(local_path("file://remote/game").is_err());
        assert!(local_path("relative/game").is_err());
    }
    #[test]
    fn saving_validates_and_preserves_previous_settings_on_failure() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().to_owned());
        paths.create_layout().unwrap();
        let mut backend = Backend {
            paths,
            settings: Settings::default(),
        };
        backend
            .save("-windowed".into(), "", "", "", false, true)
            .unwrap();
        let before = std::fs::read(&backend.paths.settings_file).unwrap();
        assert!(
            backend
                .save("changed".into(), "/missing/game", "", "", false, true)
                .is_err()
        );
        assert_eq!(before, std::fs::read(&backend.paths.settings_file).unwrap());
        assert_eq!(backend.settings.launch_options, "-windowed");
        let reloaded = Settings::load(&backend.paths.settings_file).unwrap();
        assert_eq!(reloaded.launch_options, "-windowed");
        assert!(reloaded.wine_runtime.is_none());
        assert_eq!(backend.snapshot()["runtimeAvailable"], false);
    }

    #[test]
    fn missing_wallpaper_override_falls_back_to_assets_then_palette() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().to_owned());
        paths.create_layout().unwrap();
        let backend = Backend {
            settings: Settings {
                wallpaper: Some(
                    temp.path()
                        .join("missing.png")
                        .to_string_lossy()
                        .into_owned(),
                ),
                ..Settings::default()
            },
            paths,
        };
        assert_eq!(backend.snapshot()["wallpaperUrl"], "");
        assert!(
            !backend.snapshot()["wallpaperWarning"]
                .as_str()
                .unwrap()
                .is_empty()
        );
        let asset = backend.paths.assets.join("background.jpg");
        std::fs::write(&asset, []).unwrap();
        assert_eq!(backend.snapshot()["wallpaperUrl"], file_url(&asset));
    }
}

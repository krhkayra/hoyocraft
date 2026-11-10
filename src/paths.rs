use anyhow::Result;
use directories::ProjectDirs;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub root: PathBuf,
    pub assets: PathBuf,
    pub wine: PathBuf,
    pub game: PathBuf,
    pub settings_file: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let root = std::env::var_os("HOYOCRAFT_ROOT")
            .map(PathBuf::from)
            .or_else(|| {
                ProjectDirs::from("dev", "HoYoCraft", "HoYoCraft")
                    .map(|dirs| dirs.data_dir().to_path_buf())
            })
            .ok_or_else(|| anyhow::anyhow!("Could not determine the HoYoCraft data directory"))?;

        // File dialogs and wallpaper URLs need absolute paths, including portable roots.
        let root = if root.is_absolute() {
            root
        } else {
            std::env::current_dir()?.join(root)
        };
        Ok(Self::from_root(root))
    }

    pub fn from_root(root: PathBuf) -> Self {
        Self {
            assets: root.join("assets"),
            wine: root.join("wine"),
            game: root.join("game"),
            settings_file: root.join("settings.toml"),
            root,
        }
    }

    pub fn create_layout(&self) -> Result<()> {
        for directory in [&self.root, &self.assets, &self.wine, &self.game] {
            std::fs::create_dir_all(directory)?;
        }
        Ok(())
    }

    pub fn find_default_wallpaper(&self) -> Option<PathBuf> {
        ["background.png", "background.jpg", "background.jpeg"]
            .iter()
            .map(|name| self.assets.join(name))
            .find(|path| path.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallpaper_priority_is_png_then_jpg_then_jpeg() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().to_path_buf());
        std::fs::create_dir_all(&paths.assets).unwrap();
        std::fs::write(paths.assets.join("background.jpeg"), []).unwrap();
        std::fs::write(paths.assets.join("background.jpg"), []).unwrap();
        std::fs::write(paths.assets.join("background.png"), []).unwrap();

        assert_eq!(
            paths.find_default_wallpaper().unwrap(),
            paths.assets.join("background.png")
        );
    }

    #[test]
    fn wallpaper_falls_back_to_jpeg() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().to_path_buf());
        std::fs::create_dir_all(&paths.assets).unwrap();
        std::fs::write(paths.assets.join("background.jpeg"), []).unwrap();

        assert_eq!(
            paths.find_default_wallpaper().unwrap(),
            paths.assets.join("background.jpeg")
        );
    }
}

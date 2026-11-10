use crate::{paths::AppPaths, settings::PINNED_DWPROTON_VERSION};
use serde::Serialize;
use std::{
    collections::HashSet,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize)]
pub struct Runtime {
    pub label: String,
    pub path: String,
    pub available: bool,
}

pub fn executable(path: &Path) -> bool {
    path.metadata()
        .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

/// A tool directory must contain a runnable entry point, not merely exist.
pub fn entry_point(path: &Path) -> Option<PathBuf> {
    if executable(path) {
        return Some(path.to_owned());
    }
    if !path.is_dir() {
        return None;
    }
    ["proton", "bin/wine", "files/bin/wine", "dist/bin/wine"]
        .into_iter()
        .map(|name| path.join(name))
        .find(|p| executable(p))
}

pub fn discover(paths: &AppPaths) -> Vec<Runtime> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let mut roots = vec![paths.wine.clone()];
    if let Some(home) = home {
        for suffix in [
            ".steam/root",
            ".steam/steam",
            ".local/share/Steam",
            ".var/app/com.valvesoftware.Steam/data/Steam",
        ] {
            let steam = home.join(suffix);
            roots.push(steam.join("compatibilitytools.d"));
            roots.push(steam.join("steamapps/common"));
        }
        roots.push(home.join(".local/share/lutris/runners/wine"));
    }
    let path_dirs = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    discover_in(paths, &roots, &path_dirs)
}

fn discover_in(paths: &AppPaths, roots: &[PathBuf], path_dirs: &[PathBuf]) -> Vec<Runtime> {
    let pinned = paths
        .wine
        .join(format!("dwproton-{PINNED_DWPROTON_VERSION}"));
    let mut result = vec![Runtime {
        label: format!("DWProton {PINNED_DWPROTON_VERSION} (default)"),
        path: pinned.to_string_lossy().into_owned(),
        available: entry_point(&pinned).is_some(),
    }];
    let mut seen = HashSet::new();
    if let Some(entry) = entry_point(&pinned) {
        seen.insert(entry.canonicalize().unwrap_or(entry));
    }
    let mut candidates = Vec::new();
    for root in roots {
        if let Ok(entries) = root.read_dir() {
            candidates.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
        }
    }
    for dir in path_dirs {
        candidates.push(dir.join("wine"));
        candidates.push(dir.join("wine64"));
    }
    candidates.sort();
    for path in candidates {
        if let Some(entry) = entry_point(&path) {
            let canonical = entry.canonicalize().unwrap_or(entry);
            if seen.insert(canonical) {
                result.push(Runtime {
                    label: path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                    path: path.to_string_lossy().into_owned(),
                    available: true,
                });
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    fn make_executable(path: &Path) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    #[test]
    fn discovery_pins_missing_default_and_deduplicates_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().join("data"));
        paths.create_layout().unwrap();
        make_executable(&paths.wine.join("custom/bin/wine"));
        std::os::unix::fs::symlink(paths.wine.join("custom"), paths.wine.join("alias")).unwrap();
        std::fs::create_dir(paths.wine.join("empty")).unwrap();
        let runtimes = discover_in(&paths, &[paths.wine.clone()], &[]);
        assert_eq!(runtimes.len(), 2);
        assert!(!runtimes[0].available);
        assert!(runtimes[0].path.ends_with("dwproton-11.0-3"));
        make_executable(&paths.wine.join("dwproton-11.0-3/proton"));
        assert!(discover_in(&paths, &[paths.wine.clone()], &[])[0].available);
    }
    #[test]
    fn non_executable_file_is_not_a_runtime() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("wine");
        std::fs::write(&file, "text").unwrap();
        assert!(entry_point(&file).is_none());
    }
}

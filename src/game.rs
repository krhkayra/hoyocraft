use crate::{
    paths::AppPaths,
    runtime,
    settings::{PINNED_DWPROTON_VERSION, Settings},
};
use anyhow::{Context, Result, bail};
use std::{
    ffi::OsString,
    fs::OpenOptions,
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
};

pub const GAME_DIRECTORY: &str = "Genshin Impact";
pub const GAME_EXECUTABLE: &str = "GenshinImpact.exe";

/// Accept the game library root or the named Genshin installation itself.
/// A directory containing some other executable is never treated as Genshin.
pub fn installation_directory(paths: &AppPaths, configured: Option<&str>) -> PathBuf {
    let base = configured
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.game.clone());
    if base.file_name().is_some_and(|name| name == GAME_DIRECTORY) {
        base
    } else {
        base.join(GAME_DIRECTORY)
    }
}

pub fn executable_path(paths: &AppPaths, configured: Option<&str>) -> Option<PathBuf> {
    let exe = installation_directory(paths, configured).join(GAME_EXECUTABLE);
    exe.is_file().then_some(exe)
}

fn steam_client_directory() -> OsString {
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        for suffix in [
            ".steam/root",
            ".steam/steam",
            ".local/share/Steam",
            ".var/app/com.valvesoftware.Steam/data/Steam",
        ] {
            let path = home.join(suffix);
            if path.is_dir() {
                return path.into_os_string();
            }
        }
    }
    // Proton requires the variable to exist. An empty value means there is no
    // Steam client; never create or claim an artificial Steam installation.
    OsString::new()
}

fn launch_command(paths: &AppPaths, settings: &Settings) -> Result<Command> {
    let exe = executable_path(paths, settings.game_path.as_deref()).ok_or_else(|| {
        anyhow::anyhow!(
            "Genshin Impact was not found. Expected: {}",
            installation_directory(paths, settings.game_path.as_deref())
                .join(GAME_EXECUTABLE)
                .display()
        )
    })?;
    let selected = settings
        .wine_runtime
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            paths
                .wine
                .join(format!("dwproton-{PINNED_DWPROTON_VERSION}"))
        });
    let entry = runtime::entry_point(&selected).ok_or_else(|| {
        anyhow::anyhow!(
            "Selected Wine/Proton is missing or not executable: {}",
            selected.display()
        )
    })?;
    let options = shlex::split(&settings.launch_options).ok_or_else(|| {
        anyhow::anyhow!("Launch options contain an unclosed quote or trailing escape")
    })?;
    // A selected proton symlink must keep access to its tool's adjacent files.
    let proton_name = entry.file_name().is_some_and(|name| name == "proton");
    let entry = entry
        .canonicalize()
        .context("Could not resolve the Wine/Proton executable")?;
    let proton = proton_name || entry.file_name().is_some_and(|name| name == "proton");
    let data = paths.wine.join("prefixes/genshin");
    let mut command = Command::new(entry);
    command.current_dir(exe.parent().unwrap());
    if proton {
        // Valve's entry script requires these keys and dispatches the `run` verb:
        // https://github.com/ValveSoftware/Proton/blob/master/proton
        command.arg("run");
        command.env("STEAM_COMPAT_DATA_PATH", &data);
        command.env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam_client_directory());
        command.env("STEAM_COMPAT_INSTALL_PATH", exe.parent().unwrap());
        // Clear a parent's Wine prefix: Proton chooses data/pfx itself.
        command.env_remove("WINEPREFIX");
    } else {
        // Keep the same pfx directory when changing between Wine and Proton.
        command.env("WINEPREFIX", data.join("pfx"));
    }
    command.arg(exe).args(options);
    command.stdin(Stdio::null());
    Ok(command)
}

/// Own the launcher process without blocking the GUI. Poll from a GUI timer to
/// reap it and expose exit errors; starting twice is rejected while it runs.
#[derive(Default)]
pub struct GameProcess {
    child: Option<Child>,
}

impl GameProcess {
    pub fn is_running(&self) -> bool {
        self.child.is_some()
    }

    pub fn start(&mut self, paths: &AppPaths, settings: &Settings) -> Result<u32> {
        self.poll()?;
        if self.child.is_some() {
            bail!("Genshin Impact is already running from this launcher");
        }
        let mut command = launch_command(paths, settings)?;
        let data = paths.wine.join("prefixes/genshin");
        std::fs::create_dir_all(&data)
            .context("Could not create the Genshin compatibility data directory")?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(data.join("launcher.log"))
            .context("Could not open the Genshin runtime log")?;
        command.stdout(log.try_clone()?).stderr(log);
        let child = command
            .spawn()
            .context("Could not start the selected Wine/Proton runtime")?;
        let pid = child.id();
        self.child = Some(child);
        Ok(pid)
    }

    pub fn poll(&mut self) -> Result<Option<ExitStatus>> {
        let status = match self.child.as_mut() {
            Some(child) => child
                .try_wait()
                .context("Could not check the Genshin runtime process")?,
            None => None,
        };
        if status.is_some() {
            self.child = None;
        }
        Ok(status)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        os::unix::fs::PermissionsExt,
        path::Path,
        thread,
        time::{Duration, Instant},
    };

    fn fixture() -> (tempfile::TempDir, AppPaths) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().join("root with spaces #100%"));
        paths.create_layout().unwrap();
        let game = paths.game.join(GAME_DIRECTORY);
        std::fs::create_dir(&game).unwrap();
        std::fs::write(game.join(GAME_EXECUTABLE), "fake PE").unwrap();
        (temp, paths)
    }

    fn runtime_fixture(paths: &AppPaths, name: &str) -> PathBuf {
        let tool = paths.wine.join("tool");
        std::fs::create_dir_all(&tool).unwrap();
        let path = tool.join(name);
        // Only the fixture is a shell script; application launch never invokes a shell.
        std::fs::write(&path, "#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$WINEPREFIX\" \"$STEAM_COMPAT_DATA_PATH\" \"${STEAM_COMPAT_CLIENT_INSTALL_PATH+x}\" \"$@\" > capture.txt\ni=0; while [ ! -e release ] && [ \"$i\" -lt 300 ]; do sleep 0.01; i=$((i + 1)); done\nexit 7\n").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn wait_for(path: &Path) {
        let deadline = Instant::now() + Duration::from_secs(3);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(path.exists());
    }

    #[test]
    fn detects_only_the_named_directory_and_exact_executable() {
        let (_temp, paths) = fixture();
        let expected = paths.game.join(GAME_DIRECTORY).join(GAME_EXECUTABLE);
        assert_eq!(executable_path(&paths, None), Some(expected.clone()));
        assert_eq!(
            executable_path(&paths, paths.game.to_str()),
            Some(expected.clone())
        );
        assert_eq!(
            executable_path(&paths, expected.parent().unwrap().to_str()),
            Some(expected)
        );
        std::fs::remove_file(paths.game.join(GAME_DIRECTORY).join(GAME_EXECUTABLE)).unwrap();
        std::fs::write(paths.game.join(GAME_DIRECTORY).join("launcher.exe"), []).unwrap();
        assert_eq!(executable_path(&paths, None), None);
    }

    #[test]
    fn wine_launch_preserves_arguments_and_reports_exit_without_shell_expansion() {
        let (_temp, paths) = fixture();
        let runtime = runtime_fixture(&paths, "wine");
        let settings = Settings {
            wine_runtime: Some(runtime.to_string_lossy().into_owned()),
            launch_options: "--name 'two words' '$(touch should-not-exist)'".into(),
            ..Settings::default()
        };
        let mut process = GameProcess::default();
        process.start(&paths, &settings).unwrap();
        assert!(process.is_running());
        assert!(process.start(&paths, &settings).is_err());
        let game = paths.game.join(GAME_DIRECTORY);
        wait_for(&game.join("capture.txt"));
        let capture = std::fs::read_to_string(game.join("capture.txt")).unwrap();
        let lines: Vec<_> = capture.lines().collect();
        assert_eq!(lines[0], game.to_str().unwrap());
        assert_eq!(
            lines[1],
            paths.wine.join("prefixes/genshin/pfx").to_str().unwrap()
        );
        assert_eq!(
            &lines[4..],
            &[
                game.join(GAME_EXECUTABLE).to_str().unwrap(),
                "--name",
                "two words",
                "$(touch should-not-exist)"
            ]
        );
        assert!(!game.join("should-not-exist").exists());
        std::fs::write(game.join("release"), []).unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        let status = loop {
            if let Some(status) = process.poll().unwrap() {
                break status;
            }
            assert!(Instant::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(7));
        assert!(!process.is_running());
    }

    #[test]
    fn proton_uses_verb_and_managed_compat_data() {
        let (_temp, paths) = fixture();
        let runtime = runtime_fixture(&paths, "proton");
        let settings = Settings {
            wine_runtime: Some(runtime.to_string_lossy().into_owned()),
            ..Settings::default()
        };
        let command = launch_command(&paths, &settings).unwrap();
        let args: Vec<_> = command.get_args().map(|arg| arg.to_owned()).collect();
        assert_eq!(
            args,
            [
                OsString::from("run"),
                paths
                    .game
                    .join(GAME_DIRECTORY)
                    .join(GAME_EXECUTABLE)
                    .into_os_string()
            ]
        );
        let env: std::collections::HashMap<_, _> = command
            .get_envs()
            .map(|(k, v)| (k.to_owned(), v.map(|v| v.to_owned())))
            .collect();
        assert_eq!(
            env.get(std::ffi::OsStr::new("STEAM_COMPAT_DATA_PATH")),
            Some(&Some(paths.wine.join("prefixes/genshin").into_os_string()))
        );
        assert!(env.contains_key(std::ffi::OsStr::new("STEAM_COMPAT_CLIENT_INSTALL_PATH")));
        assert_eq!(env.get(std::ffi::OsStr::new("WINEPREFIX")), Some(&None));
    }

    #[test]
    fn missing_runtime_and_bad_quotes_fail_before_creating_prefix() {
        let (_temp, paths) = fixture();
        let mut process = GameProcess::default();
        assert!(process.start(&paths, &Settings::default()).is_err());
        assert!(!paths.wine.join("prefixes").exists());
        let runtime = runtime_fixture(&paths, "wine");
        let settings = Settings {
            wine_runtime: Some(runtime.to_string_lossy().into_owned()),
            launch_options: "'unclosed".into(),
            ..Settings::default()
        };
        assert!(process.start(&paths, &settings).is_err());
        assert!(!paths.wine.join("prefixes").exists());
    }
}

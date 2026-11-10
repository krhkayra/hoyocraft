# HoYoCraft

HoYoCraft is an independent Linux launcher for HoYoverse games, written in **Rust** with a **Slint** desktop interface. It uses Slint's Qt backend to follow KDE's Breeze controls, system colors and fonts, and to provide native KDE file dialogs.

**Early development:** only Genshin Impact is currently implemented. HoYoCraft can configure and start an existing installation through Wine or Proton; it does not download or install games or compatibility runtimes. Real Genshin/DWProton compatibility has not yet been verified with a full game installation.

## Features

- A game sidebar, large wallpaper/video surface and compact native window decorations.
- Persistent launch arguments, game directory, wallpaper and Wine/Proton selection.
- Automatic detection of `Genshin Impact/GenshinImpact.exe`.
- Discovery of local, Steam, Flatpak Steam, Lutris and system Wine/Proton installations.
- A pinned **DWProton 11.0-3** default, with explicit missing-runtime feedback.
- Optional HoYoPlay static and animated backgrounds, with local fallback.
- KDE file pickers and an interface that follows the active desktop palette.

## Requirements

| Component | Purpose |
| --- | --- |
| Rust **1.92 or newer** and Cargo | Compile the Rust application and Slint 1.18.1 |
| A C++17 compiler and `pkg-config` | Compile the small native integration bridge |
| Qt **6** development files: Widgets and Multimedia, including `qmake6` | Build Slint's Qt backend and native dialogs/video support |
| Qt 6 runtime libraries and a matching display plugin | Run the desktop application |
| Plasma integration, Breeze style and Breeze icons | KDE appearance and native file dialogs |
| An existing game installation and runnable Wine/Proton | Start Genshin Impact |

The `.slint` interface, game icon and social icons are embedded in the executable. HoYoCraft does not run a QML engine. Rust dependencies are resolved through the committed `Cargo.lock`.

### Rust toolchain

Check the selected toolchain before building:

```sh
rustc --version
cargo --version
```

Use Rust 1.92 or newer. If your distribution's compiler is older, install a current stable toolchain with the [official Rust/rustup instructions](https://rust-lang.org/tools/install/). In particular, [Debian 13's stock Rust package](https://packages.debian.org/trixie/rustc) is older than this requirement. With rustup installed, `rustup update stable` updates the stable toolchain; make sure the intended `rustc` and `cargo` are on your `PATH`.

### Distribution packages

Install these system packages **in addition to the Rust toolchain above**. The commands include the Qt development libraries and the KDE runtime integration used by HoYoCraft. Wayland plugins are included; they may be omitted on an X11-only system.

#### Arch Linux

```sh
sudo pacman -Syu --needed base-devel git pkgconf \
    qt6-base qt6-multimedia qt6-multimedia-ffmpeg qt6-wayland \
    plasma-integration breeze breeze-icons qt6-svg qt6-imageformats
```

Arch includes development headers in its main Qt packages. `qt6-multimedia-ffmpeg` explicitly selects a video backend for [Qt Multimedia](https://archlinux.org/packages/extra/x86_64/qt6-multimedia/). KDE integration and style are supplied by [plasma-integration](https://archlinux.org/packages/extra/x86_64/plasma-integration/) and [breeze](https://archlinux.org/packages/extra/x86_64/breeze/).

#### Debian 13 / Trixie or newer

```sh
sudo apt update
sudo apt install build-essential git pkgconf \
    qt6-base-dev qt6-base-dev-tools qmake6 qt6-multimedia-dev qt6-wayland \
    plasma-integration kde-style-breeze breeze-icon-theme \
    qt6-svg-plugins qt6-image-formats-plugins
```

Debian 13 provides the Qt 6 generation of [plasma-integration](https://packages.debian.org/trixie/plasma-integration) and [kde-style-breeze](https://packages.debian.org/trixie/kde-style-breeze). Its development packages are [qt6-base-dev](https://packages.debian.org/trixie/qt6-base-dev) and [qt6-multimedia-dev](https://packages.debian.org/trixie/qt6-multimedia-dev). Debian 12's Plasma 5 integration is not the target of these instructions.

#### Fedora 45

```sh
sudo dnf install git gcc-c++ pkgconf-pkg-config \
    qt6-qtbase-devel qt6-qtmultimedia-devel qt6-qtwayland \
    plasma-integration plasma-breeze-qt6 breeze-icon-theme \
    qt6-qtsvg qt6-qtimageformats
```

Both [qt6-qtbase-devel](https://packages.fedoraproject.org/pkgs/qt6-qtbase/qt6-qtbase-devel/) and [qt6-qtmultimedia-devel](https://packages.fedoraproject.org/pkgs/qt6-qtmultimedia/qt6-qtmultimedia-devel/) are required to build. [plasma-breeze-qt6](https://packages.fedoraproject.org/pkgs/plasma-breeze/plasma-breeze-qt6/) supplies the Qt 6 Breeze widget style.

Package dependencies install the corresponding runtime libraries automatically. Video support follows the Qt Multimedia backend and codecs available on the distribution. Arch and Debian package names have been checked against their official package catalogs; builds and GUI tests have so far been run on Fedora 45, Rust 1.98.1 and Qt 6.11.2.

## Build and run

```sh
git clone https://github.com/krhkayra/hoyocraft.git
cd hoyocraft
cargo run --locked
```

For an optimized build:

```sh
cargo build --release --locked
./target/release/hoyocraft
```

`qmake6` and the `Qt6Widgets` / `Qt6Multimedia` pkg-config files must be discoverable. On a system with multiple Qt versions, select Qt 6 explicitly if necessary:

```sh
QMAKE=qmake6 cargo build --release --locked
```

The native bridge links Qt 6, so do not point Slint's backend at a Qt 5 installation. HoYoCraft does not need a separate Slint editor, Slint CLI, Python runtime or QML modules to run.

## First launch and data layout

HoYoCraft creates its writable data outside the source/installation directory. By default it uses the XDG application data location, normally `~/.local/share/hoyocraft`. Set `HOYOCRAFT_ROOT` for a custom or portable location:

```sh
HOYOCRAFT_ROOT="$PWD/data" cargo run --locked
```

```text
<HoYoCraft data root>/
├── assets/
│   └── background.png | background.jpg | background.jpeg
├── wine/
│   ├── dwproton-11.0-3/
│   └── prefixes/genshin/
├── game/
│   └── Genshin Impact/
│       └── GenshinImpact.exe
└── settings.toml
```

1. Place an existing game at `game/Genshin Impact/GenshinImpact.exe`, or choose its directory in **Settings**. Both the `Genshin Impact` directory and its parent are accepted.
2. Install a Wine/Proton runtime and select it in Settings. The pinned default is expected at `wine/dwproton-11.0-3/`.
3. Add launch arguments or select a wallpaper if needed, then **Save**.
4. **Launch Game** becomes available when both the exact executable and a runnable selected runtime are present.

Settings and Refresh are at the bottom of the sidebar. **Save** validates paths and atomically persists `settings.toml`; **Cancel** and closing the settings window discard the draft. Malformed existing settings are preserved and reported. Settings currently apply globally; per-game profiles are not implemented.

Launch arguments are parsed as quoted arguments without a shell. Shell expansion, commands and environment assignments are not evaluated. Local wallpaper overrides support PNG, JPEG and WebP; clearing the override restores `assets/background.png`, then `.jpg`, then `.jpeg`, or the desktop palette. Images preserve their aspect ratio and crop to fill the game area.

## Wine and Proton

The default runtime is pinned to **DWProton 11.0-3** from [Dawn Winery](https://dawn.wine/dawn-winery/dwproton/releases). HoYoCraft does not currently bundle, download, verify or update its files. A missing default stays selected with a warning; the launcher does not silently switch runtimes.

Runtime discovery checks:

- The data root's `wine/` subdirectories.
- Steam `compatibilitytools.d/` and `steamapps/common/` under `~/.steam/root`, `~/.steam/steam`, `~/.local/share/Steam`, and Flatpak Steam's `~/.var/app/com.valvesoftware.Steam/data/Steam`.
- `~/.local/share/lutris/runners/wine/`.
- `wine` and `wine64` on `PATH`.

Symlink aliases are deduplicated. Tool directories must contain executable `proton`, `bin/wine`, `files/bin/wine` or `dist/bin/wine`. Custom Steam library locations can be selected manually. Discovery never executes runtimes to determine their versions.

Wine receives the game executable directly; Proton receives `run <executable>`, following [Valve's Proton entry script](https://github.com/ValveSoftware/Proton/blob/master/proton). The working directory is the game directory. Compatibility data is kept at `wine/prefixes/genshin`, with Wine using `pfx/` inside it. Runtime output is appended to `wine/prefixes/genshin/launcher.log`.

The launcher tracks its runtime child and prevents duplicate starts while that child runs. It does not detect a detached game process or games started by another launcher instance. Game download/installation, runtime installation and anti-cheat bypass are not implemented.

## Experimental online backgrounds

Enable **Experimental: use HoYoPlay backgrounds** in Settings to fetch Genshin's `getAllGameBasicInfo` metadata. This is **off by default**. The endpoint is described in the [HoYoPlay endpoint reference](https://gist.github.com/DynamiByte/0ad250bbe1930e2736a6d4e6e842bcb2).

- With animation enabled, Qt Multimedia plays `video.url` muted and looped, transferring decoded frames to Slint.
- With animation disabled, the launcher uses `background.url` as a static image.
- A custom local wallpaper takes priority and suppresses the metadata request.
- Metadata, image and video failures fall back to the available static image, local wallpaper/assets or system palette.

The metadata request runs on a worker with a 15-second timeout and 1 MiB response limit. API theme data never changes desktop appearance. Media is not bundled or permanently cached by HoYoCraft. The API is undocumented and experimental, so its schema, availability and regional behavior may change.

## KDE behavior

The application explicitly selects Slint's `qt` backend and widget style. It follows the active QStyle, palette and font rather than installing its own light/dark theme. Native `QFileDialog` pickers delegate to KDE's `KFileWidget` through Plasma integration. `QT_QPA_PLATFORMTHEME` defaults to `kde` when unset; explicit overrides are respected.

The extra application toolbar is absent; the window manager supplies the titlebar. No default application/window icon is assigned, and the desktop file omits `Icon`. A compositor may independently show a generic fallback. The game and social artwork is navigation content, with provenance recorded in [assets/README.md](assets/README.md).

The X, Instagram and GitHub buttons link to the maintainer's profiles and open the system browser only when clicked. `data/dev.hoyocraft.HoYoCraft.desktop` supplies the desktop identity for packaging; install it under `share/applications/` alongside the executable.

## Development and checks

```sh
cargo fmt -- --check
cargo test --locked
cargo check --locked
cargo build --locked
```

GUI checks additionally need Python 3.11 or newer and `dbus-run-session`. The local video test needs the `ffmpeg` CLI with its `libvpx` encoder. These tools are optional development/test dependencies, not application runtime requirements.

```sh
# Isolated data: settings persistence, Cancel, native picker cancellation and fake game launch.
dbus-run-session -- python3 scripts/gui-smoke.py

# Optional screenshots and minimum-size layout check.
dbus-run-session -- python3 scripts/gui-smoke.py \
    --wallpaper /path/to/image.webp --compact --screenshots /tmp/hoyocraft-preview

# Verifies an actual KDE picker, then closes it automatically.
QT_QPA_PLATFORM=offscreen QT_QPA_PLATFORMTHEME=kde \
    dbus-run-session -- target/debug/hoyocraft --native-dialog-smoke-test

# Local video decode, frame transfer, pause, stop and error fallback.
dbus-run-session -- python3 scripts/media-smoke.py

# Optional network test; skipped during the normal test run.
cargo test --locked live_genshin_metadata_request -- --ignored
```

The GUI tests use temporary data and a fake runtime that records arguments; they do not start a real game or real Wine/Proton. Running `--smoke-test` directly requires an explicit `HOYOCRAFT_ROOT` and changes that root's settings, so use the script for isolation. Offscreen tests do not verify compositor decorations, GPU rendering, browser interaction or manual file selection.

Core state and game handling are in `src/controller.rs`, `src/backend.rs` and `src/game.rs`; interface markup is in `ui/launcher.slint`. `src/game_catalog.rs` registers the current Genshin entry. Adding another game also requires its bundled artwork, view/state and game-specific launch rules.

## License and affiliation

The source code is licensed under [GPL-3.0-or-later](LICENSE). Third-party artwork retains its respective ownership and licensing; see [assets/README.md](assets/README.md). HoYoCraft is an independent project and is not affiliated with or endorsed by HoYoverse.

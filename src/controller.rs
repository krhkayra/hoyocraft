//! Toolkit-independent launcher state. Workers return owned data; the UI timer
//! consumes it without touching Slint or Qt objects from background threads.
use crate::{
    backend::Backend,
    game::GameProcess,
    media::{self, MediaAssets},
};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::{
    io::{Cursor, Read},
    path::PathBuf,
    sync::mpsc::{self, Receiver, TryRecvError},
    time::Duration,
};
use url::Url;

pub struct Controller {
    pub backend: Option<Backend>,
    pub snapshot: Value,
    pub error: String,
    pub activity: String,
    pub media_error: String,
    pub media_loading: bool,
    pub media: Option<MediaAssets>,
    game: GameProcess,
    media_rx: Option<Receiver<Result<MediaAssets>>>,
}

impl Default for Controller {
    fn default() -> Self {
        Self {
            backend: None,
            snapshot: json!({}),
            error: String::new(),
            activity: String::new(),
            media_error: String::new(),
            media_loading: false,
            media: None,
            game: GameProcess::default(),
            media_rx: None,
        }
    }
}

impl Controller {
    pub fn new() -> Self {
        let mut controller = Self::default();
        controller.reload();
        controller
    }

    pub fn reload(&mut self) {
        match Backend::load() {
            Ok(backend) => {
                self.snapshot = backend.snapshot();
                self.backend = Some(backend);
                self.error.clear();
            }
            Err(error) => {
                self.backend = None;
                self.snapshot = json!({});
                self.error = format!("{error:#}");
            }
        }
        self.sync_media();
    }

    pub fn save(
        &mut self,
        options: String,
        game: &str,
        wallpaper: &str,
        wine: &str,
        api: bool,
        animated: bool,
    ) -> bool {
        let result = match &mut self.backend {
            Some(backend) => backend.save(options, game, wallpaper, wine, api, animated),
            None => Err(anyhow::anyhow!(
                "Settings could not be loaded; correct the reported error and retry"
            )),
        };
        match result {
            Ok(()) => {
                self.snapshot = self.backend.as_ref().unwrap().snapshot();
                self.error.clear();
                self.sync_media();
                true
            }
            Err(error) => {
                self.error = format!("{error:#}");
                false
            }
        }
    }

    pub fn launch(&mut self) -> bool {
        let result = match &self.backend {
            Some(backend) => self.game.start(&backend.paths, &backend.settings),
            None => Err(anyhow::anyhow!("Settings could not be loaded")),
        };
        match result {
            Ok(pid) => {
                self.activity =
                    format!("Genshin Impact compatibility runtime started (PID {pid}).");
                self.error.clear();
                true
            }
            Err(error) => {
                self.error = format!("{error:#}");
                false
            }
        }
    }

    pub fn running(&self) -> bool {
        self.game.is_running()
    }

    /// Returns true when a worker result or process exit changed view state.
    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        let media_result = self.media_rx.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                "Background request stopped unexpectedly"
            ))),
        });
        if let Some(result) = media_result {
            self.media_rx = None;
            self.media_loading = false;
            changed = true;
            match result {
                Ok(media) => {
                    self.media = Some(media);
                    self.media_error.clear();
                }
                Err(error) => {
                    self.media = None;
                    self.media_error = format!(
                        "HoYoPlay background unavailable; using local wallpaper. {error:#}"
                    );
                }
            }
        }
        match self.game.poll() {
            Ok(Some(exit)) => {
                self.activity = format!(
                    "Compatibility runtime exited: {exit}. Log: wine/prefixes/genshin/launcher.log"
                );
                changed = true;
            }
            Err(error) => {
                self.error = format!("{error:#}");
                changed = true;
            }
            _ => (),
        }
        changed
    }

    pub fn refresh_media(&mut self) {
        if self.media_loading {
            return;
        }
        self.media = None;
        self.sync_media();
    }

    fn api_enabled(&self) -> bool {
        self.backend.as_ref().is_some_and(|backend| {
            backend.settings.hoyoplay_backgrounds && backend.settings.wallpaper.is_none()
        })
    }

    fn sync_media(&mut self) {
        if !self.api_enabled() {
            // Dropping the receiver prevents a late response from restoring a
            // background after the user disabled the experiment or set an override.
            self.media_rx = None;
            self.media = None;
            self.media_loading = false;
            self.media_error.clear();
        } else if self.media.is_none() && self.media_rx.is_none() {
            let (tx, rx) = mpsc::channel();
            self.media_rx = Some(rx);
            self.media_loading = true;
            self.media_error.clear();
            std::thread::spawn(move || {
                let _ = tx.send(media::fetch_genshin_media());
            });
        }
    }

    pub fn video_url(&self) -> String {
        if self.api_enabled()
            && self
                .backend
                .as_ref()
                .is_some_and(|b| b.settings.animated_background)
        {
            self.media
                .as_ref()
                .map(|m| m.video_url.clone())
                .unwrap_or_default()
        } else {
            String::new()
        }
    }

    /// The primary API image is optional. A local override suppresses API media;
    /// unavailable API images fall back to the existing local wallpaper/assets.
    pub fn background_sources(&self) -> (String, String) {
        let local = self.snapshot["wallpaperUrl"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let remote = if self.api_enabled() {
            self.media
                .as_ref()
                .map(|m| m.background_url.clone())
                .unwrap_or_default()
        } else {
            String::new()
        };
        if remote.is_empty() {
            (local, String::new())
        } else {
            (remote, local)
        }
    }
}

const MAX_IMAGE_BYTES: u64 = 32 * 1024 * 1024;

/// RGBA bytes can cross the worker boundary safely; construct the Slint Image
/// from these pixels on the UI thread using SharedPixelBuffer::clone_from_slice.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub source: String,
}

#[derive(Default)]
pub struct BackgroundImageLoader {
    pub image: Option<DecodedImage>,
    pub error: String,
    pub loading: bool,
    requested: (String, String),
    receiver: Option<Receiver<Result<(DecodedImage, String)>>>,
}

impl BackgroundImageLoader {
    /// Safe to call each UI tick: unchanged sources do not restart a request.
    pub fn sync(&mut self, primary: &str, fallback: &str) {
        let requested = (primary.to_owned(), fallback.to_owned());
        if self.requested == requested {
            return;
        }
        self.requested = requested.clone();
        self.receiver = None;
        self.image = None;
        self.error.clear();
        self.loading = false;
        if primary.is_empty() && fallback.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.loading = true;
        std::thread::spawn(move || {
            let _ = tx.send(load_with_fallback(&requested.0, &requested.1));
        });
    }

    /// Returns true if a completed request changed the displayed image/error.
    pub fn poll(&mut self) -> bool {
        let result = self.receiver.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(result) => Some(result),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err(anyhow::anyhow!(
                "Background image loader stopped unexpectedly"
            ))),
        });
        let Some(result) = result else {
            return false;
        };
        self.receiver = None;
        self.loading = false;
        match result {
            Ok((image, warning)) => {
                self.image = Some(image);
                self.error = warning;
            }
            Err(error) => {
                self.image = None;
                self.error = format!("Wallpaper unavailable; using the desktop palette. {error:#}");
            }
        }
        true
    }
}

fn load_with_fallback(primary: &str, fallback: &str) -> Result<(DecodedImage, String)> {
    match load_image(primary) {
        Ok(image) => Ok((image, String::new())),
        Err(error) if !fallback.is_empty() && fallback != primary => {
            let image = load_image(fallback).context("The fallback wallpaper also failed")?;
            Ok((
                image,
                format!("Online wallpaper unavailable; using local wallpaper. {error:#}"),
            ))
        }
        Err(error) => Err(error),
    }
}

fn load_image(source: &str) -> Result<DecodedImage> {
    if source.is_empty() {
        bail!("No image source was provided");
    }
    let bytes = if source.starts_with("https:") {
        let parsed = Url::parse(source)?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            bail!("Wallpaper URLs must not contain credentials");
        }
        let client = reqwest::blocking::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .redirect(reqwest::redirect::Policy::limited(3))
            .build()?;
        let response = client.get(parsed).send()?.error_for_status()?;
        if response
            .content_length()
            .is_some_and(|length| length > MAX_IMAGE_BYTES)
        {
            bail!("Wallpaper exceeded the download limit");
        }
        let mut bytes = Vec::new();
        response.take(MAX_IMAGE_BYTES + 1).read_to_end(&mut bytes)?;
        bytes
    } else {
        let path = if source.starts_with("file:") {
            Url::parse(source)?
                .to_file_path()
                .map_err(|_| anyhow::anyhow!("Wallpaper must be a local file or HTTPS URL"))?
        } else {
            PathBuf::from(source)
        };
        if !path.is_absolute() {
            bail!("Wallpaper must be an absolute local file or HTTPS URL");
        }
        if std::fs::metadata(&path)?.len() > MAX_IMAGE_BYTES {
            bail!("Wallpaper exceeded the file size limit");
        }
        std::fs::read(path)?
    };
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        bail!("Wallpaper exceeded the size limit");
    }
    let mut reader = image::ImageReader::new(Cursor::new(bytes)).with_guessed_format()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let pixels = reader
        .decode()
        .context("Could not decode wallpaper image")?
        .into_rgba8();
    Ok(DecodedImage {
        width: pixels.width(),
        height: pixels.height(),
        rgba: pixels.into_raw(),
        source: source.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{paths::AppPaths, settings::Settings};

    fn fixture_controller() -> (tempfile::TempDir, Controller) {
        let temp = tempfile::tempdir().unwrap();
        let paths = AppPaths::from_root(temp.path().to_owned());
        paths.create_layout().unwrap();
        let backend = Backend {
            paths,
            settings: Settings::default(),
        };
        let snapshot = backend.snapshot();
        (
            temp,
            Controller {
                backend: Some(backend),
                snapshot,
                ..Controller::default()
            },
        )
    }

    #[test]
    fn failed_save_keeps_previous_settings_and_view() {
        let (_temp, mut controller) = fixture_controller();
        assert!(controller.save("--before".into(), "", "", "", false, false));
        let before = controller.snapshot.clone();
        assert!(!controller.save("--changed".into(), "/missing/genshin", "", "", false, true));
        assert_eq!(controller.snapshot, before);
        assert_eq!(
            controller.backend.as_ref().unwrap().settings.launch_options,
            "--before"
        );
        assert!(!controller.error.is_empty());
    }

    #[test]
    fn disabling_media_drops_late_responses_and_video() {
        let (_temp, mut controller) = fixture_controller();
        let (tx, rx) = mpsc::channel();
        controller.media_rx = Some(rx);
        controller.media_loading = true;
        controller.media = Some(MediaAssets {
            video_url: "https://example.com/video.webm".into(),
            background_url: "https://example.com/image.webp".into(),
        });
        controller.sync_media();
        assert!(tx.send(Ok(MediaAssets::default())).is_err());
        assert!(!controller.media_loading);
        assert!(controller.media.is_none());
        assert!(controller.video_url().is_empty());
    }

    #[test]
    fn local_override_suppresses_online_media() {
        let (temp, mut controller) = fixture_controller();
        let wallpaper = temp.path().join("image.png");
        let backend = controller.backend.as_mut().unwrap();
        backend.settings.hoyoplay_backgrounds = true;
        backend.settings.wallpaper = Some(wallpaper.to_string_lossy().into_owned());
        controller.snapshot = backend.snapshot();
        controller.media = Some(MediaAssets {
            video_url: "https://example.com/video.webm".into(),
            background_url: "https://example.com/image.webp".into(),
        });
        assert!(controller.video_url().is_empty());
        assert!(controller.background_sources().1.is_empty());
    }

    #[test]
    fn image_loader_falls_back_to_local_and_handles_special_paths() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("背景 #100%.png");
        image::RgbaImage::from_pixel(2, 1, image::Rgba([1, 2, 3, 255]))
            .save(&path)
            .unwrap();
        let source = Url::from_file_path(&path).unwrap().to_string();
        let (decoded, warning) = load_with_fallback("/missing/image.png", &source).unwrap();
        assert_eq!((decoded.width, decoded.height), (2, 1));
        assert_eq!(decoded.rgba, [1, 2, 3, 255, 1, 2, 3, 255]);
        assert_eq!(decoded.source, source);
        assert!(!warning.is_empty());
        assert!(load_image("http://example.com/insecure.png").is_err());
    }
}

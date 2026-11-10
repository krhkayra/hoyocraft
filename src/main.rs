mod backend;
mod controller;
mod game;
mod game_catalog;
mod media;
mod native;
mod paths;
mod runtime;
mod settings;

use controller::{BackgroundImageLoader, Controller};
use slint::{ComponentHandle, Model, ModelRc, Rgba8Pixel, SharedPixelBuffer, VecModel};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    time::Duration,
};
slint::include_modules!();

struct AppState {
    controller: Controller,
    background: BackgroundImageLoader,
    image_source: (String, String),
    video: cxx::UniquePtr<native::ffi::NativeVideo>,
    video_source: String,
    video_error: String,
}

fn pixels(bytes: &[u8], width: u32, height: u32) -> slint::Image {
    slint::Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        bytes, width, height,
    ))
}

fn theme_icon(name: &str) -> slint::Image {
    let rgba = native::ffi::theme_icon(name, 24);
    if rgba.len() == 24 * 24 * 4 {
        pixels(&rgba, 24, 24)
    } else {
        slint::Image::default()
    }
}

fn render(ui: &LauncherWindow, app: &AppState) {
    let ctl = &app.controller;
    let ready = ctl.snapshot["gameAvailable"].as_bool().unwrap_or(false);
    let runtime = ctl.snapshot["runtimeAvailable"].as_bool().unwrap_or(false);
    ui.set_can_launch(ready && runtime);
    ui.set_running(ctl.running());
    ui.set_status_text(
        if ctl.running() {
            "Game is running"
        } else if !ready {
            "Game not found"
        } else if !runtime {
            "Compatibility runtime missing"
        } else {
            "Ready to play"
        }
        .into(),
    );
    let selected = ctl.snapshot["runtimePath"].as_str().unwrap_or_default();
    let label = ctl.snapshot["runtimes"]
        .as_array()
        .and_then(|r| r.iter().find(|r| r["path"].as_str() == Some(selected)))
        .and_then(|r| r["label"].as_str())
        .unwrap_or_default();
    ui.set_runtime_label(label.into());
    ui.set_error_text(ctl.error.clone().into());
    ui.set_activity(ctl.activity.clone().into());
    ui.set_game_executable(
        ctl.snapshot["gameExecutable"]
            .as_str()
            .unwrap_or_default()
            .into(),
    );
    ui.set_loading(ctl.media_loading || app.background.loading);
    let message = if !app.video_error.is_empty() {
        &app.video_error
    } else if !app.background.error.is_empty() {
        &app.background.error
    } else {
        &ctl.media_error
    };
    ui.set_media_message(message.clone().into());
}

fn fill_settings(settings: &SettingsWindow, ctl: &Controller) {
    let snapshot = &ctl.snapshot;
    settings.set_game_path(snapshot["gamePath"].as_str().unwrap_or_default().into());
    settings.set_launch_options(
        snapshot["settings"]["launch_options"]
            .as_str()
            .unwrap_or_default()
            .into(),
    );
    settings.set_wallpaper_path(
        snapshot["settings"]["wallpaper"]
            .as_str()
            .unwrap_or_default()
            .into(),
    );
    settings.set_runtime_path(snapshot["runtimePath"].as_str().unwrap_or_default().into());
    settings.set_hoyoplay_backgrounds(
        snapshot["settings"]["hoyoplay_backgrounds"]
            .as_bool()
            .unwrap_or(false),
    );
    settings.set_animated_background(
        snapshot["settings"]["animated_background"]
            .as_bool()
            .unwrap_or(true),
    );
    let entries = snapshot["runtimes"].as_array().cloned().unwrap_or_default();
    let selected = snapshot["runtimePath"].as_str().unwrap_or_default();
    let index = entries
        .iter()
        .position(|r| r["path"].as_str() == Some(selected))
        .map(|i| i as i32)
        .unwrap_or(-1);
    let labels = entries
        .iter()
        .map(|r| {
            let label = r["label"].as_str().unwrap_or_default();
            slint::SharedString::from(if r["available"].as_bool().unwrap_or(false) {
                label.to_owned()
            } else {
                format!("{label} — missing")
            })
        })
        .collect::<Vec<_>>();
    settings.set_runtime_labels(ModelRc::new(VecModel::from(labels)));
    settings.set_runtime_index(index);
    settings.set_error_text(ctl.error.clone().into());
}

fn snapshot(ui: &impl ComponentHandle, destination: &Path) -> anyhow::Result<()> {
    let image = ui.window().take_snapshot()?;
    image::save_buffer(
        destination,
        image.as_bytes(),
        image.width(),
        image.height(),
        image::ColorType::Rgba8,
    )?;
    Ok(())
}

fn finish_smoke(code: &Rc<Cell<i32>>, result: anyhow::Result<()>) {
    if let Err(error) = result {
        eprintln!("Slint GUI smoke failed: {error:#}");
        code.set(2);
    }
    let _ = slint::quit_event_loop();
}

thread_local! {
    static UI_ACTIONS: RefCell<std::collections::VecDeque<Box<dyn FnOnce()>>> = RefCell::new(std::collections::VecDeque::new());
    static IN_UI_ACTION: Cell<bool> = const { Cell::new(false) };
}

// Qt may synchronously dispatch mouse-leave/timer events while a modal window
// changes visibility. Run native window/dialog actions outside Slint's input
// and timer callbacks; the queued closure itself is Send and captures no Rc.
fn run_ui(action: impl FnOnce() + 'static) {
    if IN_UI_ACTION.with(Cell::get) {
        action();
        return;
    }
    UI_ACTIONS.with(|queue| queue.borrow_mut().push_back(Box::new(action)));
    slint::invoke_from_event_loop(|| {
        let action = UI_ACTIONS.with(|queue| queue.borrow_mut().pop_front());
        if let Some(action) = action {
            IN_UI_ACTION.with(|active| active.set(true));
            action();
            IN_UI_ACTION.with(|active| active.set(false));
        }
    })
    .expect("Qt event loop is available");
}

fn queued(callback: impl Fn() + 'static) -> impl Fn() {
    let callback = Rc::new(callback);
    move || {
        let callback = callback.clone();
        run_ui(move || callback());
    }
}

fn queued_arg<T: 'static>(callback: impl Fn(T) + 'static) -> impl Fn(T) {
    let callback = Rc::new(callback);
    move |value| {
        let callback = callback.clone();
        run_ui(move || callback(value));
    }
}

fn after(delay: Duration, callback: impl FnOnce() + 'static) {
    slint::Timer::single_shot(delay, move || run_ui(callback));
}

fn main() -> anyhow::Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let smoke = args.iter().any(|a| a == "--smoke-test");
    if smoke && std::env::var_os("HOYOCRAFT_ROOT").is_none() {
        anyhow::bail!("--smoke-test requires isolated HOYOCRAFT_ROOT; use scripts/gui-smoke.py");
    }
    native::ffi::prepare_platform();
    slint::BackendSelector::new()
        .backend_name("qt".into())
        .select()?;
    native::ffi::configure_application(env!("CARGO_PKG_VERSION"));
    if args.iter().any(|a| a == "--native-dialog-smoke-test") {
        std::process::exit(native::ffi::native_dialog_smoke_test());
    }
    if let Some(index) = args.iter().position(|a| a == "--video-smoke-test") {
        let valid = args
            .get(index + 1)
            .ok_or_else(|| anyhow::anyhow!("Missing valid video fixture"))?;
        let invalid = args
            .get(index + 2)
            .ok_or_else(|| anyhow::anyhow!("Missing invalid video fixture"))?;
        std::process::exit(native::ffi::video_smoke_test(valid, invalid));
    }
    let ui = LauncherWindow::new()?;
    let settings = SettingsWindow::new()?;
    let details = DetailsWindow::new()?;
    ui.window().set_size(slint::LogicalSize::new(1180., 740.));
    settings
        .window()
        .set_size(slint::LogicalSize::new(660., 780.));
    details
        .window()
        .set_size(slint::LogicalSize::new(560., 220.));
    let sync_font = |ui: &LauncherWindow, settings: &SettingsWindow, details: &DetailsWindow| {
        let appearance: serde_json::Value =
            serde_json::from_str(&native::ffi::appearance()).unwrap_or_default();
        let family = appearance["fontFamily"].as_str().unwrap_or_default();
        ui.set_system_font(family.into());
        settings.set_system_font(family.into());
        details.set_system_font(family.into());
    };
    sync_font(&ui, &settings, &details);
    let font_timer = slint::Timer::default();
    font_timer.start(slint::TimerMode::Repeated, Duration::from_secs(1), {
        let ui = ui.as_weak();
        let settings = settings.as_weak();
        let details = details.as_weak();
        move || {
            if let (Some(ui), Some(settings), Some(details)) =
                (ui.upgrade(), settings.upgrade(), details.upgrade())
            {
                sync_font(&ui, &settings, &details);
            }
        }
    });
    if args.iter().any(|a| a == "--smoke-compact") {
        ui.window().set_size(slint::LogicalSize::new(820., 560.));
    }
    let app = Rc::new(RefCell::new(AppState {
        controller: Controller::new(),
        background: BackgroundImageLoader::default(),
        image_source: (String::new(), String::new()),
        video: native::ffi::create_video(),
        video_source: String::new(),
        video_error: String::new(),
    }));
    let picking = Rc::new(Cell::new(false));
    ui.set_games(ModelRc::new(VecModel::from(
        game_catalog::GAMES
            .iter()
            .map(|g| {
                let decoded = image::load_from_memory(g.artwork)
                    .expect("embedded game icon")
                    .into_rgba8();
                GameEntry {
                    key: g.id.into(),
                    name: g.title.into(),
                    artwork: pixels(decoded.as_raw(), decoded.width(), decoded.height()),
                }
            })
            .collect::<Vec<_>>(),
    )));
    let icons = [
        include_bytes!("../assets/social/x.svg").as_slice(),
        include_bytes!("../assets/social/instagram.svg").as_slice(),
        include_bytes!("../assets/social/github.svg").as_slice(),
    ];
    ui.set_socials(ModelRc::new(VecModel::from(
        game_catalog::SOCIAL_LINKS
            .iter()
            .zip(icons)
            .map(|(link, icon)| SocialEntry {
                name: link.title.into(),
                url: link.url.into(),
                icon: slint::Image::load_from_svg_data(icon).expect("embedded social icon"),
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_refresh_icon(theme_icon("view-refresh"));
    ui.set_settings_icon(theme_icon("settings-configure"));
    ui.set_play_icon(theme_icon("media-playback-start"));
    render(&ui, &app.borrow());

    ui.on_select_game({
        let weak = ui.as_weak();
        move |index| {
            if let Some(ui) = weak.upgrade() {
                if index >= 0 && (index as usize) < game_catalog::GAMES.len() {
                    ui.set_selected_game(index);
                }
            }
        }
    });
    ui.on_settings_clicked(queued({
        let app = app.clone();
        let weak = settings.as_weak();
        let main = ui.as_weak();
        move || {
            if let (Some(settings), Some(ui)) = (weak.upgrade(), main.upgrade()) {
                app.borrow_mut().controller.reload();
                fill_settings(&settings, &app.borrow().controller);
                render(&ui, &app.borrow());
                if let Err(error) = settings.show() {
                    eprintln!("Could not open settings: {error}");
                }
                native::ffi::set_window_parent("HoYoCraft Settings", "HoYoCraft", true);
            }
        }
    }));
    settings.on_cancel_clicked(queued({
        let weak = settings.as_weak();
        move || {
            if let Some(settings) = weak.upgrade() {
                let _ = settings.hide();
            }
        }
    }));
    settings.on_save_clicked(queued({
        let app = app.clone();
        let weak = settings.as_weak();
        let main = ui.as_weak();
        move || {
            if let (Some(settings), Some(ui)) = (weak.upgrade(), main.upgrade()) {
                let result = app.borrow_mut().controller.save(
                    settings.get_launch_options().to_string(),
                    &settings.get_game_path(),
                    &settings.get_wallpaper_path(),
                    &settings.get_runtime_path(),
                    settings.get_hoyoplay_backgrounds(),
                    settings.get_animated_background(),
                );
                settings.set_error_text(app.borrow().controller.error.clone().into());
                render(&ui, &app.borrow());
                if result {
                    let _ = settings.hide();
                }
            }
        }
    }));
    settings.on_runtime_selected({
        let weak = settings.as_weak();
        let app = app.clone();
        move |index| {
            if let Some(settings) = weak.upgrade() {
                if let Some(path) = app.borrow().controller.snapshot["runtimes"]
                    .as_array()
                    .and_then(|r| r.get(index as usize))
                    .and_then(|r| r["path"].as_str())
                {
                    settings.set_runtime_path(path.into());
                }
            }
        }
    });
    for kind in 0..3 {
        let weak = settings.as_weak();
        let picking = picking.clone();
        let app = app.clone();
        let callback = queued(move || {
            if let Some(settings) = weak.upgrade() {
                if picking.replace(true) {
                    return;
                }
                app.borrow_mut().video.pin_mut().pause(true);
                let current = match kind {
                    0 => settings.get_game_path(),
                    1 => settings.get_wallpaper_path(),
                    _ => settings.get_runtime_path(),
                };
                // No Rust state borrow is held during QFileDialog's nested event loop.
                let selected = match kind {
                    0 => native::ffi::pick_directory(&current),
                    1 => native::ffi::pick_wallpaper(&current),
                    _ => native::ffi::pick_runtime(&current),
                };
                picking.set(false);
                if !selected.is_empty() {
                    match kind {
                        0 => settings.set_game_path(selected.into()),
                        1 => settings.set_wallpaper_path(selected.into()),
                        _ => settings.set_runtime_path(selected.into()),
                    }
                }
            }
        });
        match kind {
            0 => settings.on_browse_game(callback),
            1 => settings.on_browse_wallpaper(callback),
            _ => settings.on_browse_runtime(callback),
        }
    }
    ui.on_refresh_clicked(queued({
        let app = app.clone();
        let weak = ui.as_weak();
        move || {
            let mut data = app.borrow_mut();
            data.controller.reload();
            data.controller.refresh_media();
            data.video.pin_mut().stop();
            data.video_source.clear();
            data.video_error.clear();
            data.background = BackgroundImageLoader::default();
            data.image_source = (String::new(), String::new());
            if let Some(ui) = weak.upgrade() {
                ui.set_video_visible(false);
                ui.set_video_frame(slint::Image::default());
                ui.set_background_image(slint::Image::default());
                render(&ui, &data);
            }
        }
    }));
    ui.on_launch_clicked(queued({
        let app = app.clone();
        let weak = ui.as_weak();
        move || {
            app.borrow_mut().controller.launch();
            if let Some(ui) = weak.upgrade() {
                render(&ui, &app.borrow());
            }
        }
    }));
    ui.on_social_clicked(queued_arg({
        let app = app.clone();
        let weak = ui.as_weak();
        move |url: slint::SharedString| {
            // Open only user-configured registry links; metadata cannot launch URLs.
            if game_catalog::SOCIAL_LINKS
                .iter()
                .any(|link| link.url == url.as_str())
                && !native::ffi::open_url(&url)
            {
                app.borrow_mut().controller.error = "Could not open the default browser".into();
                if let Some(ui) = weak.upgrade() {
                    render(&ui, &app.borrow());
                }
            }
        }
    }));
    ui.on_details_clicked(queued({
        let weak = details.as_weak();
        let app = app.clone();
        move || {
            if let Some(details) = weak.upgrade() {
                let (executable, activity) = {
                    let data = app.borrow();
                    (
                        data.controller.snapshot["gameExecutable"]
                            .as_str()
                            .unwrap_or_default()
                            .to_owned(),
                        if data.controller.activity.is_empty() {
                            "No game started in this session.".to_owned()
                        } else {
                            data.controller.activity.clone()
                        },
                    )
                };
                details.set_executable(executable.into());
                details.set_activity(activity.into());
                let _ = details.show();
                native::ffi::set_window_parent("Game details", "HoYoCraft", false);
            }
        }
    }));
    ui.window().on_close_requested({
        let settings = settings.as_weak();
        let details = details.as_weak();
        move || {
            let settings = settings.clone();
            let details = details.clone();
            run_ui(move || {
                if let Some(settings) = settings.upgrade() {
                    let _ = settings.hide();
                }
                if let Some(details) = details.upgrade() {
                    let _ = details.hide();
                }
            });
            slint::CloseRequestResponse::HideWindow
        }
    });

    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, Duration::from_millis(33), {
        let app = app.clone();
        let weak = ui.as_weak();
        let picking = picking.clone();
        move || {
            if picking.get() {
                return;
            }
            let Some(ui) = weak.upgrade() else { return };
            let mut data = app.borrow_mut();
            data.controller.poll();
            let source = data.controller.background_sources();
            if source != data.image_source {
                data.background.sync(&source.0, &source.1);
                data.image_source = source;
                ui.set_background_image(slint::Image::default());
            }
            if data.background.poll() {
                if let Some(image) = &data.background.image {
                    if smoke {
                        eprintln!("Slint wallpaper ready: {}", image.source);
                    }
                    ui.set_background_image(pixels(&image.rgba, image.width, image.height));
                }
            }
            let video_source = data.controller.video_url();
            if video_source != data.video_source {
                data.video.pin_mut().stop();
                data.video_source = video_source.clone();
                data.video_error.clear();
                ui.set_video_visible(false);
                ui.set_video_frame(slint::Image::default());
                if !video_source.is_empty() {
                    data.video.pin_mut().start(&video_source);
                }
            }
            if !data.video_source.is_empty() && data.video_error.is_empty() {
                data.video
                    .pin_mut()
                    .pause(!native::ffi::application_visible("HoYoCraft"));
                let error = data.video.error();
                if !error.is_empty() {
                    data.video_error =
                        format!("Video unavailable; using static wallpaper. {error}");
                    data.video.pin_mut().stop();
                    ui.set_video_visible(false);
                } else {
                    let frame = data.video.pin_mut().take_frame();
                    let (width, height) = (data.video.width(), data.video.height());
                    if width > 0
                        && height > 0
                        && frame.len() == width as usize * height as usize * 4
                    {
                        ui.set_video_frame(pixels(&frame, width as u32, height as u32));
                        ui.set_video_visible(true);
                    }
                }
            }
            render(&ui, &data);
        }
    });
    let exit_code = Rc::new(Cell::new(0));
    if smoke {
        eprintln!(
            "Slint frontend, Qt backend; appearance: {}",
            native::ffi::appearance()
        );
        let weak = ui.as_weak();
        let settings_weak = settings.as_weak();
        let app = app.clone();
        let code = exit_code.clone();
        let screenshot_dir = args
            .iter()
            .find_map(|a| a.strip_prefix("--smoke-screenshots="))
            .map(std::path::PathBuf::from);
        after(Duration::from_millis(600), move || {
            let result = (|| -> anyhow::Result<()> {
                let ui = weak
                    .upgrade()
                    .ok_or_else(|| anyhow::anyhow!("Missing main window"))?;
                let settings = settings_weak
                    .upgrade()
                    .ok_or_else(|| anyhow::anyhow!("Missing settings"))?;
                anyhow::ensure!(
                    ui.get_games().row_count() == 1 && ui.get_socials().row_count() == 3,
                    "Registry mismatch"
                );
                ui.invoke_settings_clicked();
                anyhow::ensure!(settings.window().is_visible(), "Settings did not open");
                anyhow::ensure!(
                    native::ffi::window_is_modal("HoYoCraft Settings"),
                    "Settings did not register as a native modal window"
                );
                let original = settings.get_game_path();
                native::ffi::cancel_next_dialog();
                settings.invoke_browse_game();
                anyhow::ensure!(
                    settings.get_game_path() == original,
                    "Cancel in native picker changed the draft"
                );
                let wallpaper = settings.get_wallpaper_path();
                settings.set_launch_options("--hoyocraft-smoke-saved".into());
                settings.set_animated_background(false);
                settings.invoke_save_clicked();
                anyhow::ensure!(
                    !settings.window().is_visible(),
                    "Save failed: {}",
                    settings.get_error_text()
                );
                ui.invoke_settings_clicked();
                anyhow::ensure!(
                    settings.get_launch_options() == "--hoyocraft-smoke-saved"
                        && settings.get_game_path() == original
                        && settings.get_wallpaper_path() == wallpaper
                        && !settings.get_animated_background(),
                    "Persistence failed"
                );
                settings.set_launch_options("--must-not-be-saved".into());
                settings.invoke_cancel_clicked();
                app.borrow_mut().controller.reload();
                anyhow::ensure!(
                    app.borrow().controller.snapshot["settings"]["launch_options"]
                        == "--hoyocraft-smoke-saved",
                    "Cancel saved draft"
                );
                ui.invoke_launch_clicked();
                anyhow::ensure!(
                    app.borrow().controller.running(),
                    "Fake runtime did not launch"
                );
                Ok(())
            })();
            if result.is_err() {
                finish_smoke(&code, result);
                return;
            }
            after(Duration::from_millis(900), move || {
                let result = (|| -> anyhow::Result<()> {
                    anyhow::ensure!(
                        !app.borrow().controller.running(),
                        "Fake runtime did not exit"
                    );
                    let ui = weak
                        .upgrade()
                        .ok_or_else(|| anyhow::anyhow!("Missing UI"))?;
                    if let Some(dir) = screenshot_dir {
                        std::fs::create_dir_all(&dir)?;
                        snapshot(&ui, &dir.join("main.png"))?;
                        ui.invoke_settings_clicked();
                        let code2 = code.clone();
                        let weak2 = settings_weak.clone();
                        after(Duration::from_millis(250), move || {
                            let result = weak2
                                .upgrade()
                                .ok_or_else(|| anyhow::anyhow!("Missing settings"))
                                .and_then(|settings| {
                                    snapshot(&settings, &dir.join("settings.png"))
                                });
                            finish_smoke(&code2, result);
                        });
                    } else {
                        finish_smoke(&code, Ok(()));
                    }
                    eprintln!(
                        "Slint GUI smoke passed: settings, save/reload, Cancel, registry and fake launch"
                    );
                    Ok(())
                })();
                if result.is_err() {
                    finish_smoke(&code, result);
                }
            });
        });
    }
    ui.run()?;
    drop(timer);
    if exit_code.get() != 0 {
        std::process::exit(exit_code.get());
    }
    Ok(())
}

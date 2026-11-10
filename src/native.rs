#[cxx::bridge(namespace = "hoyocraft")]
pub mod ffi {
    unsafe extern "C++" {
        include!("application.h");
        #[rust_name = "prepare_platform"]
        fn preparePlatform();
        #[rust_name = "configure_application"]
        fn configureApplication(version: &str);
        fn appearance() -> String;
        #[rust_name = "pick_directory"]
        fn pickDirectory(initial: &str) -> String;
        #[rust_name = "pick_wallpaper"]
        fn pickWallpaper(initial: &str) -> String;
        #[rust_name = "pick_runtime"]
        fn pickRuntime(initial: &str) -> String;
        #[rust_name = "open_url"]
        fn openUrl(url: &str) -> bool;
        #[rust_name = "set_window_parent"]
        fn setWindowParent(child_title: &str, parent_title: &str, modal: bool);
        #[rust_name = "application_visible"]
        fn applicationVisible(title: &str) -> bool;
        #[rust_name = "window_is_modal"]
        fn windowIsModal(title: &str) -> bool;
        #[rust_name = "theme_icon"]
        fn themeIcon(name: &str, size: i32) -> Vec<u8>;
        #[rust_name = "native_dialog_smoke_test"]
        fn nativeDialogSmokeTest() -> i32;
        #[rust_name = "cancel_next_dialog"]
        fn cancelNextDialog();

        type NativeVideo;
        #[rust_name = "create_video"]
        fn createVideo() -> UniquePtr<NativeVideo>;
        fn start(self: Pin<&mut NativeVideo>, source: &str);
        fn pause(self: Pin<&mut NativeVideo>, paused: bool);
        fn stop(self: Pin<&mut NativeVideo>);
        #[rust_name = "take_frame"]
        fn takeFrame(self: Pin<&mut NativeVideo>) -> Vec<u8>;
        fn width(self: &NativeVideo) -> i32;
        fn height(self: &NativeVideo) -> i32;
        fn error(self: &NativeVideo) -> String;
        #[rust_name = "video_smoke_test"]
        fn videoSmokeTest(valid_source: &str, invalid_source: &str) -> i32;
    }
}

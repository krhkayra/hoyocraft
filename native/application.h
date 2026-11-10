#pragma once

#include <cstdint>
#include <memory>
#include "rust/cxx.h"

namespace hoyocraft {

// Slint's Qt backend creates and owns QApplication. Call preparePlatform before
// selecting that backend, then configureApplication before creating windows.
void preparePlatform();
void configureApplication(rust::Str version);
rust::String appearance();
rust::String pickDirectory(rust::Str initial);
rust::String pickWallpaper(rust::Str initial);
rust::String pickRuntime(rust::Str initial);
bool openUrl(rust::Str url);
void setWindowParent(rust::Str childTitle, rust::Str parentTitle, bool modal);
bool applicationVisible(rust::Str title);
bool windowIsModal(rust::Str title);
rust::Vec<std::uint8_t> themeIcon(rust::Str name, std::int32_t size);
int nativeDialogSmokeTest();
void cancelNextDialog();

// The player and its sink live exclusively on the Qt/Slint UI thread. Rust
// polls the latest decoded RGBA frame without involving QML or Qt Quick.
class NativeVideo {
public:
    NativeVideo();
    ~NativeVideo();
    void start(rust::Str source);
    void pause(bool paused);
    void stop();
    rust::Vec<std::uint8_t> takeFrame();
    std::int32_t width() const;
    std::int32_t height() const;
    rust::String error() const;
    std::uint64_t frameCount() const;

private:
    struct Private;
    std::unique_ptr<Private> data_;
};

std::unique_ptr<NativeVideo> createVideo();
int videoSmokeTest(rust::Str validSource, rust::Str invalidSource);
}

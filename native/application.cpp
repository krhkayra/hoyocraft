#include "application.h"

#include <QtCore/QDebug>
#include <QtCore/QDir>
#include <QtCore/QEventLoop>
#include <QtCore/QJsonDocument>
#include <QtCore/QJsonObject>
#include <QtCore/QTimer>
#include <QtCore/QUrl>
#include <QtGui/QDesktopServices>
#include <QtGui/QFontInfo>
#include <QtGui/QIcon>
#include <QtGui/QImage>
#include <QtGui/QWindow>
#include <QtWidgets/QApplication>
#include <QtWidgets/QFileDialog>
#include <QtWidgets/QStyle>
#include <QtMultimedia/QAudioOutput>
#include <QtMultimedia/QMediaPlayer>
#include <QtMultimedia/QVideoFrame>
#include <QtMultimedia/QVideoSink>

namespace hoyocraft {
namespace {
QString string(rust::Str value) {
    return QString::fromUtf8(value.data(), static_cast<qsizetype>(value.size()));
}
rust::String rustString(const QString &value) {
    const auto bytes = value.toUtf8();
    return rust::String(bytes.constData(), static_cast<std::size_t>(bytes.size()));
}
QString localPath(rust::Str initial) {
    const QString path = string(initial);
    const QUrl url(path);
    return url.isLocalFile() ? url.toLocalFile() : path;
}
void parentToActiveWindow(QFileDialog &dialog) {
    if (auto *parent = QGuiApplication::focusWindow()) {
        dialog.winId();
        dialog.windowHandle()->setTransientParent(parent);
    }
    dialog.setWindowModality(Qt::ApplicationModal);
    dialog.setSupportedSchemes({QStringLiteral("file")});
}
}

void preparePlatform() {
    if (qEnvironmentVariableIsEmpty("QT_QPA_PLATFORMTHEME")) {
        qputenv("QT_QPA_PLATFORMTHEME", "kde");
    }
}

void configureApplication(rust::Str version) {
    Q_ASSERT(qApp);
    qApp->setApplicationName(QStringLiteral("HoYoCraft"));
    qApp->setOrganizationName(QStringLiteral("HoYoCraft"));
    qApp->setApplicationVersion(string(version));
    qApp->setDesktopFileName(QStringLiteral("dev.hoyocraft.HoYoCraft"));
    qApp->setWindowIcon(QIcon());
}

rust::String appearance() {
    const auto palette = QApplication::palette();
    const QFontInfo font(QApplication::font());
    const auto background = palette.color(QPalette::Window);
    QJsonObject values {
        {QStringLiteral("window"), background.name(QColor::HexArgb)},
        {QStringLiteral("text"), palette.color(QPalette::WindowText).name(QColor::HexArgb)},
        {QStringLiteral("base"), palette.color(QPalette::Base).name(QColor::HexArgb)},
        {QStringLiteral("alternateBase"), palette.color(QPalette::AlternateBase).name(QColor::HexArgb)},
        {QStringLiteral("button"), palette.color(QPalette::Button).name(QColor::HexArgb)},
        {QStringLiteral("buttonText"), palette.color(QPalette::ButtonText).name(QColor::HexArgb)},
        {QStringLiteral("highlight"), palette.color(QPalette::Highlight).name(QColor::HexArgb)},
        {QStringLiteral("highlightedText"), palette.color(QPalette::HighlightedText).name(QColor::HexArgb)},
        {QStringLiteral("disabledText"), palette.color(QPalette::Disabled, QPalette::WindowText).name(QColor::HexArgb)},
        {QStringLiteral("fontFamily"), font.family()},
        {QStringLiteral("fontPixelSize"), font.pixelSize()},
        {QStringLiteral("style"), QApplication::style()->objectName()},
        {QStringLiteral("iconEmpty"), QApplication::windowIcon().isNull()},
        {QStringLiteral("isDark"), background.lightnessF() < 0.5}
    };
    const auto json = QJsonDocument(values).toJson(QJsonDocument::Compact);
    return rust::String(json.constData(), static_cast<std::size_t>(json.size()));
}

rust::String pickDirectory(rust::Str initial) {
    QFileDialog dialog(nullptr, QApplication::translate("HoYoCraft", "Choose game directory"), localPath(initial));
    dialog.setAcceptMode(QFileDialog::AcceptOpen);
    dialog.setFileMode(QFileDialog::Directory);
    dialog.setOption(QFileDialog::ShowDirsOnly, true);
    parentToActiveWindow(dialog);
    return dialog.exec() == QDialog::Accepted ? rustString(dialog.selectedFiles().value(0)) : rust::String();
}

rust::String pickWallpaper(rust::Str initial) {
    QFileDialog dialog(nullptr, QApplication::translate("HoYoCraft", "Choose wallpaper"), localPath(initial));
    dialog.setAcceptMode(QFileDialog::AcceptOpen);
    dialog.setFileMode(QFileDialog::ExistingFile);
    dialog.setNameFilter(QApplication::translate("HoYoCraft", "Images (*.png *.jpg *.jpeg *.webp *.PNG *.JPG *.JPEG *.WEBP)"));
    parentToActiveWindow(dialog);
    return dialog.exec() == QDialog::Accepted ? rustString(dialog.selectedFiles().value(0)) : rust::String();
}

rust::String pickRuntime(rust::Str initial) {
    QFileDialog dialog(nullptr, QApplication::translate("HoYoCraft", "Choose Wine / Proton executable"), localPath(initial));
    dialog.setAcceptMode(QFileDialog::AcceptOpen);
    dialog.setFileMode(QFileDialog::ExistingFile);
    parentToActiveWindow(dialog);
    return dialog.exec() == QDialog::Accepted ? rustString(dialog.selectedFiles().value(0)) : rust::String();
}

bool openUrl(rust::Str input) {
    const QUrl url(string(input), QUrl::StrictMode);
    if (!url.isValid() || url.host().isEmpty() || !url.userName().isEmpty() || !url.password().isEmpty()
        || (url.scheme() != QStringLiteral("https") && url.scheme() != QStringLiteral("http"))) {
        return false;
    }
    return QDesktopServices::openUrl(url);
}

void setWindowParent(rust::Str childTitle, rust::Str parentTitle, bool modal) {
    const auto childName = string(childTitle);
    const auto parentName = string(parentTitle);
    QWindow *parent = nullptr;
    for (auto *window : QGuiApplication::topLevelWindows()) {
        if (window->title() == parentName) parent = window;
    }
    if (!parent) return;
    for (auto *widget : QApplication::topLevelWidgets()) {
        if (widget->windowTitle() != childName) continue;
        if (widget->windowHandle() == parent) return;
        const auto modality = modal ? Qt::WindowModal : Qt::NonModal;
        const bool remap = widget->isVisible() && widget->windowModality() != modality;
        // QWidget registers modal windows when shown; changing the QWindow
        // property on an already shown widget alone does not do this reliably.
        // Call this helper only from a queued Qt task, outside Slint callbacks.
        if (remap) widget->hide();
        widget->setWindowModality(modality);
        widget->winId();
        widget->windowHandle()->setTransientParent(parent);
        if (remap) widget->show();
        return;
    }
}

bool windowIsModal(rust::Str title) {
    const auto name = string(title);
    for (auto *widget : QApplication::topLevelWidgets()) {
        if (widget->windowTitle() != name) continue;
        const auto *window = widget->windowHandle();
        return widget->windowModality() == Qt::WindowModal
            && window && window->transientParent()
            && QGuiApplication::modalWindow() == window;
    }
    return false;
}

bool applicationVisible(rust::Str title) {
    const auto name = string(title);
    for (auto *window : QGuiApplication::topLevelWindows()) {
        if (window->title() == name) {
            return window->isVisible() && !(window->windowState() & Qt::WindowMinimized);
        }
    }
    return false;
}

rust::Vec<std::uint8_t> themeIcon(rust::Str name, std::int32_t size) {
    rust::Vec<std::uint8_t> result;
    if (size <= 0 || size > 256) return result;
    const auto icon = QIcon::fromTheme(string(name));
    if (icon.isNull()) return result;
    QImage image = icon.pixmap(size, size).toImage();
    if (image.isNull()) return result;
    image = image.scaled(size, size, Qt::IgnoreAspectRatio, Qt::SmoothTransformation)
                 .convertToFormat(QImage::Format_RGBA8888);
    const auto count = static_cast<std::size_t>(image.sizeInBytes());
    result.reserve(count);
    const auto *source = image.constBits();
    for (std::size_t i = 0; i < count; ++i) result.push_back(source[i]);
    return result;
}

void cancelNextDialog() {
    QTimer::singleShot(250, qApp, [] {
        for (auto *widget : QApplication::topLevelWidgets()) {
            if (auto *dialog = qobject_cast<QFileDialog *>(widget)) dialog->reject();
        }
    });
}

int nativeDialogSmokeTest() {
    const bool noLauncherIcon = QApplication::windowIcon().isNull();
    qInfo() << "HoYoCraft native appearance:" << QString::fromUtf8(appearance().c_str());
    QWindow parentWindow;
    parentWindow.setTitle(QStringLiteral("HoYoCraft native picker parent"));
    parentWindow.resize(400, 300);
    parentWindow.show();
    parentWindow.requestActivate();
    QCoreApplication::processEvents(QEventLoop::AllEvents);
    QFileDialog dialog(nullptr, QStringLiteral("HoYoCraft native dialog smoke test"), QDir::tempPath());
    dialog.setFileMode(QFileDialog::Directory);
    dialog.setOption(QFileDialog::ShowDirsOnly, true);
    parentToActiveWindow(dialog);
    bool foundKdePicker = false;
    QTimer::singleShot(500, &dialog, [&] {
        for (auto *widget : QApplication::allWidgets()) {
            if (widget->inherits("KFileWidget")) {
                foundKdePicker = true;
                qInfo() << "HoYoCraft native picker verified:" << widget->metaObject()->className();
            }
        }
        if (!foundKdePicker) {
            qWarning() << "HoYoCraft native dialog smoke test: no KFileWidget; check plasma-integration and QT_QPA_PLATFORMTHEME";
        }
        dialog.reject();
    });
    dialog.exec();
    return foundKdePicker && noLauncherIcon ? 0 : 2;
}

struct NativeVideo::Private {
    QVideoSink sink;
    QAudioOutput audio;
    QMediaPlayer player;
    QImage latest;
    QString failure;
    bool fresh = false;
    std::uint64_t frames = 0;

    Private() {
        audio.setMuted(true);
        player.setAudioOutput(&audio);
        player.setVideoSink(&sink);
        player.setLoops(QMediaPlayer::Infinite);
        QObject::connect(&sink, &QVideoSink::videoFrameChanged, &player, [this](const QVideoFrame &frame) {
            if (!failure.isEmpty()) return;
            QImage image = frame.toImage();
            if (image.isNull()) return;
            // Bound the decoded transfer into Slint; preserve the source ratio.
            if (image.width() > 1280 || image.height() > 900) {
                image = image.scaled(1280, 900, Qt::KeepAspectRatio, Qt::SmoothTransformation);
            }
            latest = image.convertToFormat(QImage::Format_RGBA8888);
            fresh = true;
            ++frames;
        });
        QObject::connect(&player, &QMediaPlayer::errorOccurred, &player, [this](QMediaPlayer::Error, const QString &message) {
            failure = message;
            latest = QImage();
            fresh = false;
        });
    }
};

NativeVideo::NativeVideo() : data_(std::make_unique<Private>()) {}
NativeVideo::~NativeVideo() {
    // Disconnect before member teardown: a player's shutdown can notify its sink.
    QObject::disconnect(&data_->sink, nullptr, &data_->player, nullptr);
    QObject::disconnect(&data_->player, nullptr, &data_->player, nullptr);
    data_->player.stop();
    data_->player.setVideoSink(nullptr);
    data_->player.setAudioOutput(nullptr);
}

void NativeVideo::start(rust::Str source) {
    stop();
    const auto text = string(source);
    if (text.isEmpty()) return;
    const QUrl url(text, QUrl::StrictMode);
    if (!url.isValid() || (!url.isLocalFile() && url.scheme() != QStringLiteral("https"))) {
        data_->failure = QStringLiteral("Video source must be a local file or HTTPS URL");
        return;
    }
    data_->player.setSource(url);
    data_->player.play();
}
void NativeVideo::pause(bool paused) {
    if (paused) data_->player.pause();
    else if (!data_->player.source().isEmpty() && data_->failure.isEmpty()) data_->player.play();
}
void NativeVideo::stop() {
    data_->player.stop();
    data_->player.setSource(QUrl());
    data_->latest = QImage();
    data_->failure.clear();
    data_->fresh = false;
    data_->frames = 0;
}
rust::Vec<std::uint8_t> NativeVideo::takeFrame() {
    rust::Vec<std::uint8_t> bytes;
    if (!data_->fresh || data_->latest.isNull()) return bytes;
    data_->fresh = false;
    const auto count = static_cast<std::size_t>(data_->latest.sizeInBytes());
    bytes.reserve(count);
    const auto *source = data_->latest.constBits();
    for (std::size_t i = 0; i < count; ++i) bytes.push_back(source[i]);
    return bytes;
}
std::int32_t NativeVideo::width() const { return data_->latest.width(); }
std::int32_t NativeVideo::height() const { return data_->latest.height(); }
rust::String NativeVideo::error() const { return rustString(data_->failure); }
std::uint64_t NativeVideo::frameCount() const { return data_->frames; }
std::unique_ptr<NativeVideo> createVideo() { return std::make_unique<NativeVideo>(); }

int videoSmokeTest(rust::Str validSource, rust::Str invalidSource) {
    NativeVideo video;
    QEventLoop loop;
    QTimer poll;
    int phase = 0;
    int pausedTicks = 0;
    int result = 2;
    std::uint64_t pausedFrames = 0;
    video.start(validSource);
    QObject::connect(&poll, &QTimer::timeout, &loop, [&] {
        if (phase == 0 && !video.error().empty()) {
            qWarning() << "Slint native video fixture failed:" << video.error().c_str();
            loop.quit();
        } else if (phase == 0 && video.frameCount() >= 3) {
            const auto bytes = video.takeFrame();
            if (bytes.size() != static_cast<std::size_t>(video.width() * video.height() * 4)) {
                qWarning() << "Slint video RGBA transfer size mismatch";
                loop.quit();
                return;
            }
            pausedFrames = video.frameCount();
            video.pause(true);
            phase = 1;
        } else if (phase == 1 && ++pausedTicks >= 10) {
            if (video.frameCount() > pausedFrames + 1) {
                qWarning() << "Slint native player did not pause";
                loop.quit();
                return;
            }
            video.stop();
            if (!video.takeFrame().empty() || video.width() != 0) {
                qWarning() << "Slint native player retained a stopped frame";
                loop.quit();
                return;
            }
            video.start(invalidSource);
            phase = 2;
        } else if (phase == 2 && !video.error().empty()) {
            if (!video.takeFrame().empty() || video.width() != 0) {
                qWarning() << "Slint native player retained a failed frame";
                loop.quit();
                return;
            }
            qInfo() << "Slint native video smoke passed: decoded" << pausedFrames
                    << "frames, RGBA transfer, pause, stop and error-frame clearing";
            result = 0;
            loop.quit();
        }
    });
    poll.start(50);
    QTimer::singleShot(10000, &loop, [&] { qWarning() << "Slint native video smoke timed out at phase" << phase; loop.quit(); });
    loop.exec();
    return result;
}
}

PREFIX ?= /usr
DESTDIR ?=

all:
	cargo run --build

clean:
	rm -f Cargo.lock
	rm -rf target

install:
	install -Dm755 target/release/hoyocraft $(DESTDIR)$(PREFIX)/bin/hoyocraft
	install -Dm644 assets/hoyocraft $(DESTDIR)$(PREFIX)/share/applications/hoyocraft.desktop
	install -Dm644 assets/hoyocraft.png $(DESTDIR)$(PREFIX)/share/icons/hicolor/256x256/apps/hoyocraft.png
	install -Dm644 assets/hoyocraft.png $(DESTDIR)$(PREFIX)/share/icons/hicolor/310x310/apps/hoyocraft.png

uninstall:
	rm -rf $(DESTDIR)$(PREFIX)/bin/WaifuDownloader
	rm -rf $(DESTDIR)$(PREFIX)/share/applications/hoyocraft.desktop
	rm -rf $(DESTDIR)$(PREFIX)/share/icons/hicolor/256x256/apps/hoyocraft.png
	rm -rf $(DESTDIR)$(PREFIX)/share/icons/hicolor/256x256/apps/hoyocraft.png

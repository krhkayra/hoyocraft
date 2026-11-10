#!/usr/bin/env python3
"""Run the compiled Slint GUI with isolated data and a fake Wine runtime."""
import base64
import json
import os
from pathlib import Path
import subprocess
import tempfile
import tomllib
import argparse
import shutil

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--screenshots", type=Path)
parser.add_argument("--wallpaper", type=Path, help="Optional local wallpaper for visual QA")
parser.add_argument("--compact", action="store_true", help="Check the minimum desktop window size")
args = parser.parse_args()

repo = Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="hoyocraft-gui-") as temporary:
    root = Path(temporary)
    game = root / "game alanı #100%" / "Genshin Impact"
    game.mkdir(parents=True)
    (game / "GenshinImpact.exe").write_bytes(b"fixture")
    assets = root / "assets"
    assets.mkdir()
    wallpaper = assets / ("背景 #100%" + (args.wallpaper.suffix if args.wallpaper else ".png"))
    wallpaper.write_bytes(base64.b64decode(
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGMwSJjwHwAEBAIg5PKlggAAAABJRU5ErkJggg=="
    ))
    if args.wallpaper:
        shutil.copyfile(args.wallpaper, wallpaper)
    runtime = root / "wine" / "custom" / "bin" / "wine"
    runtime.parent.mkdir(parents=True)
    runtime.write_text("#!/bin/sh\nprintf '%s\\n' \"$PWD\" \"$@\" > launch-capture.txt\nexit 0\n")
    runtime.chmod(0o755)
    # JSON quoted strings are also valid TOML basic strings for these fixture paths.
    settings = root / "settings.toml"
    settings.write_text("\n".join([
        'launch_options = "--before-smoke"',
        "game_path = " + json.dumps(str(game), ensure_ascii=False),
        "wallpaper = " + json.dumps(str(wallpaper), ensure_ascii=False),
        "wine_runtime = " + json.dumps(str(runtime), ensure_ascii=False),
    ]))
    env = dict(os.environ, HOYOCRAFT_ROOT=str(root), QT_QPA_PLATFORM="offscreen",
               SLINT_BACKEND="qt", QT_FORCE_STDERR_LOGGING="1")
    command = [str(repo / "target/debug/hoyocraft"), "--smoke-test"]
    if args.compact:
        command.append("--smoke-compact")
    if args.screenshots:
        destination = args.screenshots.resolve()
        destination.mkdir(parents=True, exist_ok=True)
        command.append("--smoke-screenshots=" + str(destination))
    subprocess.run(command,
                   env=env, check=True, timeout=45)
    saved = tomllib.loads(settings.read_text())
    assert saved["launch_options"] == "--hoyocraft-smoke-saved", saved
    assert saved["game_path"] == str(game), saved
    assert saved["wallpaper"] == str(wallpaper), saved
    assert saved["wine_runtime"] == str(runtime), saved
    assert saved["hoyoplay_backgrounds"] is False
    assert saved["animated_background"] is False
    capture = (game / "launch-capture.txt").read_text().splitlines()
    assert capture == [str(game), str(game / "GenshinImpact.exe"), "--hoyocraft-smoke-saved"], capture
    print("Slint settings persistence, Cancel and fake-runtime game launch verified.")

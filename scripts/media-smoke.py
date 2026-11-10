#!/usr/bin/env python3
"""Exercise the Slint native video bridge with local VP8/WebM fixture data.

Requires ffmpeg with the libvpx encoder and a built target/debug/hoyocraft.
No network, QML engine, real game or persistent application data is used.
"""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

repo = Path(__file__).resolve().parents[1]
binary = repo / "target/debug/hoyocraft"
if not binary.is_file() or not shutil.which("ffmpeg"):
    raise SystemExit("Run cargo build first; media-smoke.py also requires ffmpeg with libvpx")

with tempfile.TemporaryDirectory(prefix="hoyocraft-media-") as temporary:
    root = Path(temporary)
    video = root / "fixture.webm"
    subprocess.run([
        "ffmpeg", "-hide_banner", "-loglevel", "error", "-f", "lavfi",
        "-i", "color=c=blue:s=160x90:r=10", "-t", "2", "-c:v", "libvpx",
        "-an", "-y", str(video),
    ], check=True, timeout=20)
    invalid = root / "invalid.webm"
    invalid.write_bytes(b"This is an intentionally invalid video fixture.")
    env = dict(os.environ, HOYOCRAFT_ROOT=str(root), QT_QPA_PLATFORM="offscreen",
               SLINT_BACKEND="qt", QT_FORCE_STDERR_LOGGING="1")
    subprocess.run([
        str(binary), "--video-smoke-test", video.as_uri(), invalid.as_uri(),
    ], env=env, check=True, timeout=20)
    print("Slint native video playback, RGBA frames, pause, stop and error fallback verified.")

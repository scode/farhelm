#!/usr/bin/env bash
# Record the staged demo from Farhelm's desktop app. See docs/readme-video/SPEC.md.

repo="$(cd "$(dirname "$0")/.." && pwd)" || exit 1
exec python3 -B "$repo/scripts/readme_video.py" "$@"

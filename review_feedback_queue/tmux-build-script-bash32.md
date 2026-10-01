# The private tmux build script fails on stock macOS bash

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

Developer tooling only: building Farhelm's private tmux for macOS with `scripts/build-private-tmux.sh` aborts
immediately on a Mac whose default bash is the stock 3.2.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F47 / COR-TMUX-BUILD-BASH32`, tagged **possible**. Anchor and title: `scripts/build-private-tmux.sh:110` — the
private-tmux build script's macOS branch fails under stock macOS bash.

This is developer tooling only; nothing a Farhelm user runs is affected. Farhelm ships its own pinned build of tmux, and
`scripts/build-private-tmux.sh` is the script that compiles it (together with static copies of its libevent and ncurses
dependencies). The script's header documents two kinds of target: fully static Linux binaries, and an Apple arm64
(`aarch64-apple-darwin`) build.

The script runs under `set -euo pipefail`, where `-u` makes any reference to an unset variable a fatal error. It
declares a few option arrays up front as empty (`configure_host=()`, `static_link=()`), and only the Linux branch fills
them; the macOS branch leaves both empty. Later, every configure step expands them as `"${configure_host[@]}"`, and the
link flags as `${static_link[*]}` (lines 110, 111, 124 and 135). Bash older than 4.4 treats expanding an empty array as
an unbound-variable reference under `set -u`, so it aborts at the very first configure (libevent) before building
anything. macOS's own `/bin/bash` is 3.2, and the script's `#!/usr/bin/env bash` shebang picks whichever bash comes
first on `PATH`, so on a stock Mac the documented macOS build cannot run.

The impact is limited: nothing in CI or the release pipeline exercises the macOS branch (the release and CI wrappers,
`build-tmux-assets.sh` and `build-pinned-tmux-ci.sh`, only ever pass Linux targets), so it only bites someone invoking
the script by hand on a Mac without a newer bash first on `PATH`. The project already uses the portable idiom elsewhere:
`.github/workflows/sign-sums.yml` writes `${extra[@]+"${extra[@]}"}`. The suggested fix is to use that same
`${arr[@]+"${arr[@]}"}` form for the optional arrays, or to have the script check for and require bash 4.4 or newer
explicitly.

# Build outputs

Why every release attempt starts and ends with `cargo clean`, and exactly when. It is on the stable release, RC release
and dev release reading lists in `releasing/AGENTS.md`, because every release procedure cites it from its pre-tag and
closing steps.

Cargo files every workspace crate's build outputs under a hash that includes the package version, and every release
attempt changes the version. So each attempt's builds land beside the previous attempt's instead of replacing them, and
nothing ever removes the old ones. A checkout used for releases grew past 100 GB that way in about two weeks: dozens of
copies of the version-parity test binary with their incremental caches, plus the reproductions of gate failures run
between attempts. A release attempt therefore starts and ends with `cargo clean` in the checkout it is cut from:

- Right before the version-parity tests, the attempt's first build. This also clears what an earlier attempt left
  behind, including one whose session ended before its own cleanup. It costs little: the version bump rebuilds every
  workspace crate regardless, so only the dependencies are rebuilt that would otherwise have been reused.
- When the attempt is over: the release is published and verified, or the attempt is abandoned. Not while a gate failure
  is still being reproduced in that checkout, since the reproduction wants its warm build.

`cargo clean` removes the checkout's whole `target/`, including the dx bundle and any earlier development build there.
The pinned tmux and nextest (`.ci-tmux`, `.ci-nextest`) live outside it and stay.

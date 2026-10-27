# Older installer can prune a live custom-state version

Reviewed commit: 918eea3a7613d74fd8deb2886ced1d5c542626d3

## TLDR

Repeated installation of v0.27.0 can remove the executable a newer running supervisor still needs for later session
launches, when that supervisor uses another state directory.

## Details

The production handoff at `scripts/install.sh:940–975` executes published v0.27.0, whose source correspondence was
checked statically. Pruning at `scripts/fixtures/install-v0.27.0.sh:1358–1391` ignores the newer version locks and does
not discover a supervisor using another absolute state directory. The first installation keeps newer version N as
previous Installed; a second can prune N. The surviving newer supervisor still launches through its own N executable
(`crates/farhelm-supervisor/src/service/core.rs:14746–14812`; `session_hold.rs:68–91`). Later launches can therefore
fail.

Suggested bucket: `high`. Confidence: confirmed conditional source mechanism; occurrence unmeasured. No death of an
already running process, loss of session data or broader work-loss consequence was established. This is not inferred
merely from an old program lacking a new feature; the repeated-install and later-launch path is explicit.

Possible cover: trailing-tip `SPEC_impl.md:4658–4668` promises live-version protection. Required historical handoff does
not explicitly accept pruning a still-needed version. The older-release upgrade filter's carried-across-upgrade trigger
is absent from this downgrade sequence.

Provenance: `g019/f1`. Published script/checksum and Git-source correspondence were inspected without executing the
installer.

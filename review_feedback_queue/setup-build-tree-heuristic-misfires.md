# setup's build-tree check refuses installed binaries

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With TMPDIR set to an empty value, or an install path that contains a directory named target, `farhelm helm setup`
refuses to install the services and wrongly says the binary is a build artifact.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F13 / COR-BUILD-TREE-HEURISTIC`, tagged **definite**. Anchors and title: `crates/farhelm/src/setup.rs:1288`,
`crates/farhelm/src/setup.rs:1296`, `crates/farhelm/src/main.rs:2178` — setup's build-tree check refuses correctly
installed binaries under an empty TMPDIR or a path containing a "target" directory

Before writing units, setup refuses to point them at a binary that "looks like a build tree", to stop people pinning a
`cargo build` output or a copy in `/tmp` that will vanish. `looks_like_a_build_tree` (setup.rs:1287–1298) refuses when
any path component of the executable is exactly `target`, or when the executable path starts with the temporary
directory. The temp directory comes from `std::env::temp_dir()` (main.rs:2178), which returns `$TMPDIR` verbatim when it
is set, even when it is empty (confirmed in the Rust std source, `sys/paths/unix.rs`). For `TMPDIR=""`,
`canonicalize("")` fails, so the unresolved empty path is used, and `Path::starts_with("")` is true for every path:
setup refuses every binary. `TMPDIR=/`, or a relative `TMPDIR` that resolves to a parent of the install directory, does
the same.

The `target` rule catches legitimate installs too: the default install for a Unix user named `target`
(`/home/target/.local/bin/farhelm`), or `FARHELM_INSTALL_DIR=/opt/target/bin`. There is no override flag; only
`--dry-run` downgrades the refusal to a note. The message tells the operator to "Install farhelm first", which is what
they already did.

Suggested change: apply the temp-dir prefix test only when the temp dir resolves to an absolute path other than `/`;
match cargo's layout narrowly (`target/{debug,release,<triple>}/`); and treat an installer ownership record beside the
binary as proof it is installed, or add an override flag.

Restater note: the mechanism is certain, but the triggers (empty or `/` TMPDIR, a `target` component in the install
path) are uncommon.

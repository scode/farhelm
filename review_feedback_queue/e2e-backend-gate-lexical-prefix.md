# The E2E backend gate's state-dir check is lexical

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

On a release helm started with a test-only variable containing `..`, provisioning is silently faked and host paths can
come from a directory another local user controls, instead of the gate refusing to start.

## Details

Paths are relative to `crates/farhelm-helm/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0602-2b597e9-f96c` (audit of Area 8, provisioning) as
`F31 / SEC-E2E-GATE-LEXICAL`, tagged **possible**. Anchors and title: `provisioning/e2e.rs:101`,
`provisioning/service.rs:138` — The shipped E2E backend gate checks "inside the state directory" lexically, so .. or a
symlink points it anywhere

Every production helm, release builds included, checks the environment variable `FARHELM_E2E_PROVISIONING_BACKEND_DIR`
at startup (service.rs:138). When it is set, provisioning is replaced by a simulated backend used by the browser test
suite, driven by a `config.json` in that directory. Its `dial_farhelm` and `dial_state_dir` values are registered as
real host rows and then dialed over real ssh, so they choose which remote command runs as the user on the remote host.
The gate is meant to require that the directory sit inside the helm's private state directory and contain an `ENABLED`
marker file. The check is `root.is_absolute() && root.starts_with(helm_state_dir)` (e2e.rs:101). `Path::starts_with`
compares path components without resolving them, so `<state>/../../tmp/x` passes, and so does a symlink inside the state
directory that points elsewhere.

Setting the variable already requires control of the helm's own account, which SPEC.md accepts as the security boundary,
so this is not a standalone cross-account exploit. But the gate is weaker than its documentation says, and a `..` value
can make a directory another local user controls decide which remote paths this user's ssh executes. Suggested change:
canonicalize both paths before the prefix check; require the directory and marker to be owned by the current uid and not
group- or world-writable; and consider compiling the seam out of `farhelm_release_build` binaries.

User-visible consequence: on a release helm started with this test-only variable containing `..`, provisioning is
silently faked and host paths can come from a directory another local user controls, instead of the gate refusing to
start.

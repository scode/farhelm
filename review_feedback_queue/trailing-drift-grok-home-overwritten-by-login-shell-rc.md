# GROK_HOME set via seams.launch_env is applied before the login shell and can be overwritten by shell rc files

Reviewed commit: 1b3fe49af86979aebc2719965682a17f8c6366ac

## TLDR

If a maintainer configures a custom `GROK_HOME` in their login shell startup files (`~/.profile`, `~/.bashrc`, or
`~/.zshrc`), the Grok drift test launches Grok against the maintainer's real `GROK_HOME` instead of the private mirror —
silently exercising whatever hook file is in the real configuration (or failing to capture) rather than the hook file
installed by the build under test.

## Details

**other.** Source inspection. Main reference `crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:293`.

`Root::GrokHome::real` (`crates/farhelm/tests/e2e/real_agent_capture/drift/private_config.rs:60-73`) explicitly supports
a maintainer having `GROK_HOME` set, and `PrivateConfig::apply` (`private_config.rs:287-298`) passes the mirror path to
the agent by pushing `("GROK_HOME", mirror.private)` into `seams.launch_env`. However, `seams.launch_env` is applied in
`host_child_env` (`crates/farhelm-supervisor/src/service/core/host_launch.rs:141-155, 512-518`) BEFORE the terminal host
runs `window_command` (`crates/farhelm-supervisor/src/launch.rs:716-722`), which starts
`<shell> -l -i -c 'exec <farhelm> internal launch <spec>'` (as documented on `SupervisorSeams::launch_env` at
`crates/farhelm-supervisor/src/service/core.rs:1011-1015`: "so they reach the login shell before it sources anything").
When the user's shell rc files export `GROK_HOME` (the standard way a custom `GROK_HOME` is configured), `-l -i`
overwrites `GROK_HOME` back to the maintainer's real directory before `exec farhelm internal launch` spawns `grok`.
Meanwhile `Supervisor::setup_vendor_hooks` used `seams.grok_home` directly and wrote the test build's hook file into
`private-grok_home/hooks/farhelm.json`, so `private.check_installed()` at `drift.rs:681` passes even though `grok`
itself never reads the mirror. Contrast `drift.rs:640-644`, which applies `MUSE_NO_AUTO_UPDATE` and `PI_CONFIG_FILES`
through `wrapper_prefix` (`env ...`) so they take effect after login-shell startup. Verification: trace
`private_config.rs:60-73, 287-298` -> `host_launch.rs:512-518` -> `launch.rs:716-722`. Fix: apply the mirror's
`GROK_HOME` after login-shell startup as well (e.g., via `wrapper_prefix` `env GROK_HOME=...` as Muse/OMP do, or a
post-shell seam in `LaunchSpec.env`), so shell rc exports cannot redirect `grok` back to the maintainer's real
configuration.

Reviewer metadata (g13_drift_private_hooks/f4): suggested bucket: other — confined to the opt-in maintainer drift test
harness when the maintainer exports `GROK_HOME` in shell rc files.; confidence: confirmed; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g13_drift_private_hooks/f4`.

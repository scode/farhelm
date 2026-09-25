# setup pins a relative XDG_STATE_HOME

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

With a relative XDG_STATE_HOME, the services keep state in a different place than `farhelm helm token show` reads, so
the token it shows does not unlock the running helm.

## Details

Found by pre-pr-review-swarm run `20260925-0834-2b597e9-be22` (audit of Area 9, install/uninstall/setup/hooks) as
`F15 / COR-RELATIVE-XDG-STATE-HOME`, tagged **possible**. Anchors and title: `crates/farhelm/src/setup.rs:567-570` —
helm setup writes a relative XDG_STATE_HOME into both units without resolving it

Setup pins one state directory into both units. An explicit `--state-dir` is made absolute against the directory setup
ran in (`ctx.absolute`), but the default comes from `default_state_dir_for` (in farhelm-supervisor), which takes any
non-empty `XDG_STATE_HOME` and returns `<value>/farhelm`, relative or not (setup.rs:567–570). That relative path is
written into both units unchanged. systemd starts user services in the user's home directory, so the services use
`$HOME/<value>/farhelm`, while an operator command such as `farhelm helm token show`, run from some other directory,
resolves the same relative default against its own working directory and reads a different tree.

The `SetupContext` docs promise that "everything setup pins is made absolute", precisely so services and operator
commands agree on one state directory. The XDG Base Directory spec says a relative value should be ignored, which
`user_unit_dir_for` already does for `XDG_CONFIG_HOME`. Suggested change: treat a non-absolute `XDG_STATE_HOME` as
unset, matching `user_unit_dir_for`. The visible symptom is that `farhelm helm token show` prints a token that does not
unlock the running helm.

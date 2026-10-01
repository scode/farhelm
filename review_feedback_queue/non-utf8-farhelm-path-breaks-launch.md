# A non-UTF-8 farhelm or state path breaks every launch

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

On a host where the farhelm binary or the supervisor's state directory lives at a path that is not valid UTF-8, every
session create and restart fails with a confusing shell error, while the supervisor's log says it is merely running in a
degraded mode.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F16 / COR-EXE-LOSSY`, tagged **definite**. Anchor and title: `crates/farhelm-supervisor/src/launch.rs:599` — a
farhelm binary or state directory at a non-UTF-8 path makes every agent launch fail, though the code says launches still
work.

Every agent launch works the same way. The supervisor asks tmux to run a small shell command,
`exec '<farhelm binary>' internal launch '<launch spec file>'`, and the farhelm binary, run in this role (the launch
shim), then starts the actual agent. That command is built by `window_command`
(`crates/farhelm-supervisor/src/launch.rs:599-602`). It converts both the farhelm binary path and the launch-spec path
to text with `to_string_lossy()`, so any non-UTF-8 bytes become replacement characters. The shell is then asked to exec
a file that does not exist, and the shim never starts.

The rest of the supervisor claims this case is harmless. It keeps the binary path both as a raw path and as optional
text that is empty when the path is not valid UTF-8. The field's documentation
(`crates/farhelm-supervisor/src/service/core.rs:4055-4063`) says an empty text copy only means agents get no
conversation-reporting hooks, because "the shim itself is addressed by PathBuf (see window_command)". The startup
warning (lines 4926-4933) likewise says Claude "retains record scanning". Both statements are false: `window_command`
does not address the shim by raw path. The launch-spec file sits under the supervisor's state directory, which is made
absolute at startup but never checked for UTF-8, so a non-UTF-8 state directory breaks launches the same way. The
existing test for this case (`with_hook_argv_cannot_hook_a_non_utf8_farhelm_path`, around line 26415) only checks that
hooks are skipped. It never builds the launch command.

On such a host every create and restart fails with a confusing exec error from the shell, while the supervisor's own log
says it is merely running in a degraded mode. The trigger is rare. The suggested fix is to pick one rule and make the
code and docs agree. Either refuse a non-UTF-8 farhelm path or state directory at supervisor startup with a message
naming the path, which matches the helm's own rule, and delete the "degradation". Or pass the exact bytes through to the
shell and correct the docs and the warning. Add a test that launches with a non-UTF-8 executable path.

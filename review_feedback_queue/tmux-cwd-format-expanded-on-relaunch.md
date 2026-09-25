# Restart in place lets tmux format-expand the working directory

Reviewed commit: 2b597e90dcc715c5efa61e257d775725f80bd946

## TLDR

After a restart, the agent comes back in the home folder (or a crafted folder name re-runs its command).

## Details

Paths are relative to `crates/farhelm-supervisor/src/` unless they start with `crates/`.

Found by pre-pr-review-swarm run `20260925-0414-2b597e9-0497` (audit of Area 4, tmux seam) as `F2 / COR-CWD-RELAUNCH`,
tagged **definite**. Anchors and title: `tmux.rs:2658`, `service/core.rs:12384`, `service/core.rs:9733`,
`service/core.rs:9904` — Restart in place passes the working directory to `respawn-pane -c` unescaped, bypassing
restart's own directory identity check

Restarting a session normally reuses its existing tmux pane, so the terminal's scrollback and pane id survive.
`relaunch_in_pane` does this with `tmux respawn-pane -k -t =<session>:.<pane> -c <cwd> -- …`. This is a separate call
site from F1, so fixing `create_session` alone leaves it broken. `respawn-pane -c` is format-expanded exactly like
`new-session -c`. Reviewers saw `#{l:x}` resolve to `x`, and `C#Samples` land in `$HOME`. In some runs `#(touch …)` also
fired; the job runs asynchronously, so a short test can miss it.

The restart path goes to some trouble to prevent this exact outcome. Before relaunching, `restart_session` calls
`ensure_cwd_usable` and then `ensure_cwd_identity` (core.rs:9904, documented at 9733). Together they prove that the
stored path still resolves to the directory the session was created in. The restart then hands the resolved path, not
the stored spelling, to tmux, so a symlink cannot be repointed between the check and the launch. Format expansion
defeats all of that. The check validates one directory, and tmux changes into a different one, or into `$HOME`, without
an error.

Every restart or resume of such a session therefore puts the agent, including a resumed conversation, in the wrong
directory, or re-runs a `#(…)` command embedded in the directory name. Restart can be requested by the user or by an
agent through `farhelm agent restart`. The fix is the same `#` → `##` helper as F1, with the regression test also
exercising a restart.

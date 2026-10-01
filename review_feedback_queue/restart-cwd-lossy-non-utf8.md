# Restart can start the agent in the home directory

Reviewed commit: ff6b96c9c58bdbe9698cb8e9cbb08885c18abfc3

## TLDR

If a session's folder is reached through a symlink whose target name is not valid UTF-8, every restart (or retried
create) starts the agent in the user's home directory instead, often with permission-skipping flags, and reports
success.

## Details

Found by pre-pr-review-swarm run `20261001-0226-ff6b96c-5e95` (whole-repo audit, correctness and security reviewers
only) as `F15 / COR-CWD-LOSSY`, tagged **definite**. Anchor and title:
`crates/farhelm-supervisor/src/service/core.rs:2903` — restart and create-retry can start the agent in the home
directory when the working directory resolves through a symlink to a non-UTF-8 path.

When a session is created in an existing folder, the supervisor records the folder's canonical path, meaning the path
with all symlinks resolved. It does this with a lossy conversion to text (`to_string_lossy`,
`crates/farhelm-supervisor/src/service/core.rs:7183-7192`), which replaces any bytes that are not valid UTF-8 with the
Unicode replacement character. The create itself launches into the literal path the user typed, which is always valid
text, so the first launch is fine.

Restart, and the retry of a create that did not finish, re-check the folder before relaunching (`ensure_cwd_identity`,
lines 2903-2931; restart calls it at line 10048). The check resolves the path again, converts it lossily again, and
compares the result with the stored text. On a match it returns that resolved text as the directory to launch in. That
last step is deliberate: launching into the fully resolved path means nobody can swap a symlink between the check and
the launch. But if the real target has non-UTF-8 bytes in its name, the resolved text names a directory that does not
exist. Nothing checks it, and it goes to tmux as the new window's starting directory. tmux's own behaviour when it
cannot enter a window's starting directory is to fall back silently to the user's home directory, and to `/` if that
fails (this is tmux's spawn logic, outside this repository). The comparison has a second weakness: two different
non-UTF-8 targets that turn into the same replacement text compare as equal, so a repointed link can pass the check.

The trigger is rare, but the result is that every restart or retry of such a session starts the agent, often with
permission-skipping flags, in a directory nobody chose, and reports success. The check exists precisely to guarantee
that what was verified is what runs. The suggested fix is never to convert canonical paths lossily. Use the strict
conversion (`to_str()`) both at create and in the check, and refuse with a message such as "working directory resolves
to a path that is not valid UTF-8". The checkout-root resolution and the folder browser already refuse non-UTF-8 paths
in a similar way (for example around line 7290). Optionally, also confirm the launch directory exists before handing it
to tmux. Add a test with a symlink to a non-UTF-8 directory.

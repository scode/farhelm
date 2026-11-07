# Stop deletes directories it rejects as capture runs

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A mistaken directory supplied to the screen-capture stop command can cause silent recursive deletion of unrelated user
work.

## Details

Source coordinates from the original records: `scripts/agent-tui.py:134-140`; `scripts/agent-tui.py:134-141,298-318`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **highest**. Source-confirmed conditional behavior does not imply
an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the
finding.

Later-turn synthesis verified the complete CLI admission path. scripts/agent-tui.py:298-300 declares stop with an
unrestricted positional run argument; parse_args at 306-309 only normalizes start command argv, and main at 312-318
calls the selected handler without validating the stop path. stop at 134-137 converts that user string directly into
pathlib.Path and constructs HostRun. scripts/agent_screen_host.py:83-91 checks whether directory/farhelm exists and
raises HostError if it does not. Exactly that ownership rejection is caught at agent-tui.py:138; line 140 passes the
original user-supplied directory to shutil.rmtree(ignore_errors=True), and line 141 returns success. The handler has no
requirement that the directory be a random /tmp capture directory, no positive ownership check before deleting in this
branch, no confirmation, and no preserved sentinel boundary. A user-supplied existing directory containing source files
and no farhelm marker therefore reaches recursive deletion. For stop . the contents are deleted even if removing dot
itself subsequently fails and is ignored. This is direct accidental-input admission, not a hostile-local-account
scenario or a conjectured live-install mutation. The earlier parent implementation stopped at SystemExit on missing tmux
marker (2d5859c5f144e0c286a0576976aae2d1c66ddc7c^:scripts/agent-tui.py:93-97, 214-217); introduced by
2d5859c5f144e0c286a0576976aae2d1c66ddc7c and unchanged through tip.

Suggested bucket: highest, retained: the demonstrated source route permits catastrophic deletion of user-owned work;
tooling origin does not reduce that consequence. Confidence: confirmed source-level admission and destructive operation,
no destructive runtime experiment performed. proposed_drop: false. possible_cover: none; parent independently checks
exclusions. Fix should reject non-run paths without deleting them. This synthesis adds exact CLI argument/dispatch
evidence and does not broaden discovery scope.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g047/f4`, `g048/f6`.
The retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their
own revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.

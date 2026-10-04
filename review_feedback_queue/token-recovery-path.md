# Token recovery command silently targets a different path

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Token recovery command silently targets a different path.

## Details

`F18 / COR-TOKEN-RECOVERY-PATH` — **definite** — `crates/farhelm-helm/src/token_control.rs:349` — Token recovery command
silently targets a different path

When token rotation has an uncertain outcome, the CLI tells the user to run `farhelm helm token show` against the same
state directory to recover the current credential. An explicitly supplied state directory is accepted as a native path,
which can contain bytes that are not valid UTF-8. The recovery-command helper converts that path with `to_string_lossy`,
replacing undecodable bytes before producing the copyable command.

Rotation can consequently target the original directory while its recovery advice targets a different directory
containing replacement characters. Following that advice can create a token database in the wrong directory and return a
token that cannot authenticate to the original helm.

Refuse an unsupported state-directory path before rotating, or make recovery-command construction fallible and use
strict conversion with an explicit refusal. The problem requires both a non-UTF-8 explicit path and an uncertain
rotation reply.

Suggested bucket: **other**. No runtime reproduction was performed, and the original credential remains recoverable;
this does not establish permanent credential loss. The earlier non-UTF-8 sweep at `TRIAGE_OUTCOMES.md:4547–4583`
concerns other helpers. The current specification forbids acting on a corrupted path. Its allowance for lossy diagnostic
display does not cover a command that the user is instructed to execute.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `helm_systems p2`.

Possible cover recorded during collection: Ledger4547–4583 earlier nonUTF8 sweep different helper; SPEC1774–1778
prohibits corrupted path operation..

Collection caveats: Original credential not lost, no actual repro. NonUTF8 and uncertain reply required; not highest.
Wrongpath mutable command not diagnostic-only filter.

## Filed reviewer metadata

- `helm_systems p2`: confidence as filed: definite / confirmed by source and contract; conditional on an explicit
  non-UTF-8 --state-dir and an unreadable/lost online rotation reply. No runtime reproduction. Suggested bucket as
  filed: other.

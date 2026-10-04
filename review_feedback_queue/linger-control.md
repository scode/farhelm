# Remote linger failure writes terminal controls to helm logs

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Remote linger failure writes terminal controls to helm logs.

## Details

`F2 / SEC-LINGER-CONTROL` — **definite** — `crates/farhelm-helm/src/provisioning/backend.rs:2319` — Remote linger
failure writes terminal controls to helm logs

Provisioning asks the remote host to enable linger, which lets its supervisor start without an interactive login. When
that command fails in a way treated as degraded operation, the helm logs the host’s stderr using tracing’s display
formatter, `%output.stderr.trim()`.

The output capture limits size and decodes text, but neither decoding nor trimming escapes embedded terminal controls.
Display formatting preserves them in the helm’s stderr log. A remote host can therefore supply escape sequences that
repaint the operator’s terminal or forge the apparent log presentation. On terminals that support and permit OSC 52, the
same channel could affect the clipboard.

Pass this text through the existing bounded, control-escaping peer-text helper, or use a Debug-escaped representation.
Add a focused formatter regression proving that control bytes are printed as text rather than emitted literally.

Suggested bucket: **highest**. Raw control preservation is established by inspection; no exploit was run, and clipboard
effects depend on the terminal. `SPEC_impl.md:1069–1078` requires escaped peer diagnostics. The earlier logging decision
at `TRIAGE_OUTCOMES.md:721–738` accepts ordinary string fields because their Debug formatting escapes them; this
explicit Display field bypasses that protection.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `helm_security p1`, `helm_general p2`.

Possible cover recorded during collection: SPEC_impl1069–1078 requires control escaped; ledger721–738 accepts Debug not
this bypass..

Collection caveats: No exploit; clipboard terminal-dependent; raw controls established.

## Filed reviewer metadata

- `helm_general p2`: confidence as filed: definite / confirmed for control-character injection; terminal-specific
  exploit effects were not reproduced. Suggested bucket as filed: highest, security. Confidence: definite / confirmed
  for control-character injection; terminal-specific exploit effects were not reproduced.
- `helm_security p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable

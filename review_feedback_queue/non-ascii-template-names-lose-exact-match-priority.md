# Non-ASCII template names lose exact-match priority

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

A complete non-ASCII template name selects a broader match first.

## Details

F207 — **definite** — `crates/farhelm-ui/src/launch_composer.rs:1081` — Non-ASCII template names lose exact-match
priority

Template filtering and exact-match priority use different case normalization. With the supplied accented-name case, both
templates match but the intended complete name is not recognized as exact, so the earlier broader result is initially
selected. Accepting it fills the launcher with another template's settings. Use identical case normalization for
filtering and exact-match priority.

## Evidence and triage context

- crates/farhelm-ui/src/launch_composer.rs:801 filters with Unicode lowercase.
- crates/farhelm-ui/src/launch_composer.rs:1026 uses ASCII lowercase for default selection; :1081 compares template
  names with eq_ignore_ascii_case.
- crates/farhelm-proto/src/launcher.rs:320 permits the example Unicode names.
- crates/farhelm-ui/src/list/create_form.rs:4882 assigns the default index; :4950 accepts that selected result.

Retained confidence: **definite**. Suggested bucket: **other**.

Possible cover:

- No matching Planned, BUGS, queue, ledger, or filter coverage found.

Caveats:

- The mismatch requires non-ASCII case differences and an earlier matching template.
- The highlighted result is visible.
- Acceptance changes the draft; it does not itself launch a session.
- No runtime reproduction or security consequence claimed.
- The selected result remains visible.
- Acceptance populates a draft; it does not itself launch a session.
- No runtime reproduction.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_08_sec:p1:F1`.

- `ui_desktop_08_sec:p1:F1`: confidence as filed: definite; suggested bucket as filed: other.

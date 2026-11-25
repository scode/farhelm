# Hook setup hint keeps a stale Don't show again error

Reviewed commit: 74c83433cb83c60197eb33939b836bf8afa2b54a

## TLDR

If saving the conversation hook hint's "Don't show again" fails, the red error line sticks to that session for the rest
of the connection. Dismissing the hint from Settings and later turning it back on brings the hint back with the old
"Could not save hiding the hook setup prompt and hint" line, though nothing has failed since. The workspace trust hint
was fixed to drop its error in exactly these cases (6ada98e544); the hook hint was not.

## Details

NOTE: This was triaged before. The archived 2026-11-24 outcome for `hook-hint-can-repaint-old-dismissal-error.md`
(`TRIAGE_OUTCOMES_ARCHIVE.md`) decided "fix if cheap, otherwise no change", and its execution in #4863 chose no change
because a complete fix exceeded the cheap allowance. The archive is not a coverage source for recording, so the finding
is recorded again with that citation; triage can close it on the same basis if nothing has changed.

At the tip, `crates/gui/farhelm-gui/src/session_pane/hook_band.rs` sets `Known.error` only in the dismiss settle
callback or on Busy (~96-115) and clears it only at the start of the next dismiss or on a UI reset. `HookBand::follow`
(~65-81) never clears it: not when the shared dismissal is stored, not when the session is left, not when the occurrence
ends. `draw` filters the error by key and hides the band while dismissed, so the line comes back when the preference is
restored. Route: live Grok or Antigravity session with the hook hint, press Don't show again, the helm refuses, the
error shows; dismiss in Settings (hint hidden); re-enable in Settings (hint returns with the old error).

Compare `trust_band.rs` at 6ada98e544: `follow` clears `Known.failed` when dismissed, when the open key changes, or when
`read_for != wanted`, and `hide_settled` drops a refusal that lands while the hint is dismissed. SPEC.md's Claude trust
hint paragraph now says "A stored dismissal also drops the error" and "leaving the session drops the error". The hook
hint paragraph says nothing about the error's lifetime, so the hook band is unspecified either way.

Fix: in `HookBand::follow`, clear the error when the stored dismissal is true for the errored agent, when the selection
leaves the errored key, or when the row no longer yields a hint; in the settle callback, drop a refusal when the hint is
already dismissed. Mirror `a_refused_dont_show_again_says_so_only_while_its_session_is_open` for the hook band.

Suggested bucket: `other` (misleading stale error on a dismissible hint, no data or security effect). Confidence:
confirmed from source, not executed. Possible cover: the archived outcome named above.

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g01-hint-bands-hooks` record `f1`.

# Restart with wake failures keep launch-only remedies

Reviewed commit: 7dcb9b33d12d528ff6149ebc0de1b6b834b7cab7

## TLDR

When Restart with has to wake a suspended sandbox and the host turns out to point at a different installation (or drops
out after reconnecting), the launcher now says "Restart error." but follows it with launch wording: "The host now points
to a different installation; choose it again before launching" or "The host became unavailable before the session could
launch". Restart with cannot change its host, so the first remedy is impossible. The next Restart press shows the
correct restart-worded refusal, so the wrong text lasts only until then.

## Details

7dcb9b33d1 removes the "Not launched. " prefix from the wake failure sentences (`launch/wake.rs:149-151` and
`failure_words` at `wake.rs:276-278`) and has the launcher prepend the mode's lead at render (`view.rs:2770-2774`,
`2799-2803`). The reasons are still New/Clone wording: `wake.rs:211-213`
`Advance::Fail("The host now points to a different installation; choose it again before launching")` and `wake.rs:218`
`Advance::Fail("The host became unavailable before the session could launch")`. Restart with holds its host and
destination (`draft.rs` `holds(Field::Host)`/`holds(Field::Destination)`), and the draft's own refusal for the same
condition already uses restart wording (`request.rs` `host_refusal`: "The source host installation cannot be confirmed;
close and reopen restart with" / "The source host is unavailable; wait for it to reconnect before restarting"). Same
text at the tip (`wake.rs:212`, `218`).

Fix: pass the mode into the wake (or choose the sentence in `wake_refused`) so Restart with gets the restart wording
`host_refusal` already uses; extend the new `launcher_restart_wake` golden or a wake test to cover a `Fail` reason.

Suggested bucket: `other` (misleading text on a rare failure path; nothing is restarted). Confidence: confirmed from
source; the triggers are rare. The wording predates the commit, but this commit set out to make these failures read as
restart errors (SPEC.md "Restart error. for Restart with").

Provenance: trailing code review through `953603f1`, run `fee35ad2-3831-4a91-b555-fe8cb1f8f1ae`, reviewer
`g14-launcher-wake-hooks` record `f1`.

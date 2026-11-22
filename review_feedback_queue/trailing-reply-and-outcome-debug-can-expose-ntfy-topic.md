# UI channel Reply and operations::Outcome derive Debug over JSON bodies that carry ntfy topic

Reviewed commit: 66ad3aaa95ef03a2130e2d0194d58353912ae9da

## TLDR

Farhelm deliberately omits `Debug` on the notification settings struct so diagnostics never include a user's private
ntfy topic, but the generic UI-channel reply and GUI operation outcome wrappers derive `Debug` over raw JSON bodies that
carry that topic. No shipped production log call currently debug-formats either type, so this is a hardening gap rather
than an active leak.

## Details

**other.** Source inspection.

Separate lead reported during the Batch 1 independent drop check (Entry D8, cid `04ea9818-2964-458c-810b-52a3f533e21a`).
Source references at `66ad3aaa95ef03a2130e2d0194d58353912ae9da`: `crates/farhelm-proto/src/ntfy.rs:44-53` omits `Debug`
on `ntfy::Settings` with the comment `"Topic is editable configuration, but must never be included in diagnostics"`
(`SPEC_impl.md:5950`: `"No diagnostic includes the topic, token or title"`). `GET/PUT /api/ntfy-alerts`
(`crates/farhelm-helm/src/ntfy.rs:397-426`) returns `ntfy::Settings` in the HTTP response body, so `topic` travels
inside `Reply::body` (`crates/farhelm-proto/src/ui_channel.rs:188-212`) and `Outcome::Done`
(`crates/gui/farhelm-gui/src/operations.rs:598-603`), both of which currently derive `Debug`. Bucket: `other` (hardening
gap; no production `tracing::` call currently debug-formats `Reply` or `Outcome`). Confidence: confirmed by source
inspection for the `Debug` derives; no active leak path established. Fix direction: implement custom `Debug` on `Reply`
and `Outcome` that redacts `body`, matching `farhelm_proto::ui_channel::Command` (`ui_channel.rs:95-105`).

Review evidence is retained under run `34570dac-b635-4682-a942-3a3fc1728e12` (independent drop check Batch 1, cid
`04ea9818-2964-458c-810b-52a3f533e21a`).

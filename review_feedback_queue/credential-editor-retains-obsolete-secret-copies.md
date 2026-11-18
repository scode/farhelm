# The credential editor keeps obsolete copies of a draft secret

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The credential editor can keep an extra copy of a rejected or superseded token or login file in app memory after you
have edited the draft.

## Details

Haiku pass H-P7-8, checked by gpt-6.2-sol. Suggested queue bucket **other**. Confirmed additional in-process retention
on both routes. No new reader outside the app process or disclosure was established.

- Verdict: NOT CRITICAL
- Route checked: `settings/credentials/editor.rs:59–79,162–173,236–239,324–331`; `SPEC.md:2338–2340`.
- Reason: The native error record stores a plaintext copy to match the draft. Editing the draft hides the error while
  the copy remains until another action clears it or the editor is dropped. The old editor invalidated errors on input.
  This lengthens in-process retention, but the copy has no established reader outside the already trusted app process.

- Verdict: NOT CRITICAL
- Route checked: `settings/credentials/editor.rs:71–76,232–233,334–386`; `SPEC_impl.md` under Named credential catalog.
- Reason: The native task retains the pre-chooser Codex draft through file selection and reading, even if intervening
  edits supersede it. This comparison implements stale-read rejection with a full secret copy where an edit generation
  would suffice. It stays inside the app and is discarded when the task ends or is abandoned.

Use an edit generation instead of retaining plaintext for refusal identity and chooser freshness. Clear obsolete refusal
data on input and keep stale chooser completions from winning.

Possible cover: the write-only field in SPEC.md:2333–2342 permits holding the current draft. It leaves obsolete
in-process copies unspecified. `gui-command-debug-includes-secret-body.md`, `codex-login-file-rendered-in-cleartext.md`,
and `masked-fields-expose-secrets-to-text-services.md` cover different readers and routes.

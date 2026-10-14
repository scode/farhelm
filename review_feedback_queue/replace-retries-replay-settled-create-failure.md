# Replace retries retain a permanently failed creation key

Reviewed commit: fcbcbe26286604c115fd2a7e64a85f34dcb4ac9d

## TLDR

After fixing the cause of a failed Replace, an ordinary retry can keep returning the old failure instead of starting a
replacement.

## Details

Suggested bucket: `high`. Confidence: confirmed static trace; no runtime reproduction. Source: dedicated `gpt-6.1-sol`
high adversarial review. Proposed drop: none.

`crates/farhelm-ui/src/api.rs:1751–1764` retains non-authentication refusals without the definitely-unaccepted header.
Ordinary plain-Replace validation errors lack that header: `crates/farhelm-helm/src/lib.rs:2231–2236` marks only
`FreshCreateUnaccepted`, checkout conflicts or YOLO confirmation.

For example, temporarily remove an existing session's folder, try Replace, then restore the folder and retry. The
supervisor validates the folder (`crates/farhelm-supervisor/src/service/core.rs:7303–7306`), records its failure
(`7156–7190,8500–8519`; `crates/farhelm-supervisor/src/store.rs:3676–3717`) and replays it on the same key before
revalidating (`core.rs:7133–7140,8194–8196,14028–14032`). Plain Replace uses permanent keys
(`crates/farhelm-helm/src/sessions.rs:1702–1706,3247`; supervisor `service/handlers.rs:66–73`). Restoring the folder
therefore cannot repair an ordinary retry, in either sidebar or header. Cancelling a reopened confirmation, switching
controls or remounting can obtain a fresh key, but the refusal does not explain that requirement.

The first parent minted a fresh key on each attempt (`api.rs:2633`); this commit introduces the persistent failure.
Propagate durable non-acceptance for ordinary creates and retire their settled failed keys, while retaining ambiguous
outcomes and partial-success intents.

Possible cover: durable supervisor refusal replay is intentional, but does not explicitly accept keeping an acknowledged
failed create unresolved in the UI. TRIAGE_OUTCOMES.md:12839–12843 requires a fresh key for a deliberate new Replace
after an acknowledged result. The rare self-correcting failure filter does not match ordinary retries that indefinitely
replay the failure after its cause is repaired.

Current-main status: persists at `4c9e9612d217be9f2ffe4d586c6bffaf8e1cd703`; UI classification is at `api.rs:1984–1997`,
the helm's marker condition at `lib.rs:2259–2264`, and supervisor recording/replay at
`core.rs:7143–7145,8505–8524,14072–14076`. Verify with an owned session folder: observe a settled failed create, restore
the folder, retry Replace and require a new successful replacement without discarding the whole view.

# Release capture accepts an unloaded account form

Reviewed commit: dc40aac5b56310f31df2f65c93695bfe88538392

## TLDR

The Tensorlake release screenshot can report success while its API-key form is still loading or has failed to load. Its
readiness assertion accepts a read-only input. This does not establish that an existing published image is wrong.

## Details

Suggested bucket: **other**. Confidence: confirmed source-level readiness defect; no delayed/failed capture was run and
frequency is unmeasured.

`e2e/docs-shots/release-notes.spec.ts:37–44` opens Settings, selects Tensorlake, waits for its unconditional
experimental warning, then checks `toBeEnabled()` on the key input. The shared opener at `593–598` waits only for sound
controls. The account independently loads its settings (`crates/farhelm-ui/src/tensorlake_settings.rs:98–114`), defines
locked as not loaded or busy (`145–148`), and renders the input read-only rather than disabled (`183–202`). It is
therefore enabled while unloaded and after a failed read. Every Settings section mounts even when hidden
(`settings.rs:623–632`). The final screenshot helper checks leaks and nonempty crops, not loaded settings
(`e2e/docs-shots/shot.ts:83–116`).

SPEC_impl.md, Tensorlake account, `5984–6037` describes this independent read and section lifetime. The test-authoring
contract calls for an observable witness of the premise. Wait for editability and the intended loaded no-key-saved
fixture state before annotating or capturing; a delay would not establish readiness.

Possible cover: SPEC.md, Documentation scope, `20–22` freezes published screenshots, and the ordinary helper skips a
published shot. This finding concerns the executable script during initial capture or deliberate recapture, not a
request to refresh frozen images. No exact specification acceptance, plan, queue, ledger or filter coverage was found.
There is no security, user-work-loss or catastrophic tooling consequence established.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g001/g001-l002`. Proposed drop: none.

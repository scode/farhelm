# Choosing Grok preserves incompatible workspace trust

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Choosing Grok preserves incompatible workspace trust.

## Details

`F13 / COR-GROK-TRUST` — **definite** — `crates/farhelm-proto/src/launcher.rs:107` — Choosing Grok preserves
incompatible workspace trust

The shared launcher transition is supposed to clear choices that the newly selected agent type cannot use. When
switching a launch with an explicit workspace-trust choice from Codex, Muse, or Pi to Grok, it takes an early return
that clears model and effort but leaves workspace trust intact. The trust-clearing code lies after that return.

CLI template application uses this transition. For example, a trusted Codex template followed by an edit or another
template selecting Grok retains the old trust value. The helm then rejects the resolved launch because Grok does not
offer workspace trust. A supported agent-type change therefore produces an invalid launch instead of clearing the
incompatible choice.

Clear unsupported workspace trust before the early return, or make all transition branches pass through the same
normalization. Cover both a direct transition and stacked templates.

Suggested bucket: **high**; the underlying reviews differed between **other** and **high**. No possible cover was
identified.

Restater note: The shared-helper and CLI/template defect is confirmed. The browser also calls the shared helper, but
independently normalizes workspace trust when constructing its submitted launch. Browser launch rejection from this
defect was therefore not confirmed and should not be asserted from the shared call alone.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `helm_edges p1`, `helm_general p2`.

Possible cover recorded during collection: none identified.

Collection caveats: GUI shared function traced by general; edges says CLI proven and GUI separately normalizes; check
before stating GUI reachability. Suggested buckets other/high differ.

Coordinator confirmation: retain the shared-helper and CLI/template defect. The browser normalizes the submitted trust
independently, so browser rejection is excluded.

## Filed reviewer metadata

- `helm_general p2`: confidence as filed: definite / confirmed by the complete selection-to-validation trace. Suggested
  bucket as filed: high, material UX. Confidence: definite / confirmed by the complete selection-to-validation trace.
- `helm_edges p1`: confidence as filed: original confidence wording unavailable Suggested bucket as filed: original
  suggested-bucket wording unavailable

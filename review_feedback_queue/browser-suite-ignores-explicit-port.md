# The browser suite ignores its documented port override

Reviewed commit: e79077f5a0f2cd229a9feefa5f5cf7edf67f905e

## TLDR

A maintainer choosing the browser-test port with `FARHELM_E2E_PORT` silently gets a different address. Manual probes at
the requested port miss the stack. Ordinary recorded runs remain internally consistent.

## Details

`e2e/playwright.config.ts:18–26` now calls the slot resolver (`e2e/stack-slots.ts:52–61`), whose plural port helper
reads only `FARHELM_E2E_STACK_PORTS` before allocating fresh ports (`e2e/stack-port.ts:74–84`). The config sends its
selected port to the shell (`182–185`). The singular helper still promises stable manual probing
(`stack-port.ts:44–52`), and `start-stack.sh:7–10` still says suite configuration accepts that input. The later
three-worker revision does not restore the interface. Preserve compatibility where meaningful or reject the old input
clearly and document migration.

Suggested bucket: **other**. Confidence: **confirmed ignored input and retained claim; external consumers beyond the
documented manual workflow unverified**. Recorded runs scrub unrequested ambient Farhelm overrides, and the selected
browser and shell ports agree. This is a direct maintainer-interface concern, not ordinary recorded-run breakage.
Proposed drop: none.

Possible cover: deliberate migration to the plural variable is plausible but no explicit acceptance was found in either
SPEC, Planned TODOs, BUGS, queue or ledger. Deterministic ignored configuration does not clearly match a rare-glitch
filter. Provenance: browser-harness tests `tests-1/f1`, contracts and sibling test/implementation corroboration in the
trailing review through `fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `browser-harness-implementation-1/f1`, `browser-harness-tests-1/f1`, `browser-harness-tests-2/f1`,
`browser-harness-tests-3/f4`, `browser-harness-tests-4/f5`, `browser-harness-contracts-1/f4`.

# Changed-template regression accepts a missing retry key

Reviewed commit: 6d0b152493a31bcde6af09f149cc9f5a83287b0f

## TLDR

The changed-template regression can pass if the fresh launch has no retry key, leaving it unable to detect lost
duplicate protection. No production omission was established.

## Details

`e2e/tests/quick-launch.spec.ts:1178` only requires the changed request's `intent_key` to differ from the first. Missing
or null satisfies that assertion. The following successful-response and two-session checks (`1180–1182`) do not reject
this counterexample: the server accepts an absent key as an independent create
(`crates/farhelm-helm/src/sessions.rs:1001–1006,1535–1547` at the reviewed revision). Require a nonempty fresh key
before comparing it, or lose the changed launch's reply and verify another retry recovers that same launch.

Suggested bucket: **other**. Confidence: **confirmed static oracle gap; no runtime mutation test performed**. This is
weakened test coverage, not a demonstrated duplicate product launch. Proposed drop: none.

Possible cover: final `TRIAGE_OUTCOMES.md:17281–17301`, `unchanged-template-drops-launch-retry-key.md`, explicitly
requires changed requests to mint a fresh identity. The older permission-choice decision at `10396–10419` has a
different trigger. `SPEC.md:845–855`, `SPEC_impl.md:5066–5095`, Planned TODOs, BUGS, queue and FILTER supply no exact
exclusion. Provenance: `launch-retries-tests-1/f1`, discovery pass 1 in the trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `launch-retries-tests-1/f1`.

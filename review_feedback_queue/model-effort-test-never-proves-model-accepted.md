# Model-effort regression never proves model acceptance

Reviewed commit: 120b17803c80248a062308fee4a1966761f8bebd

## TLDR

The effort-preservation regression can pass when Enter fails to accept the custom model. It does not independently prove
that the operation whose effect it tests occurred. No current product failure was established.

## Details

`e2e/tests/custom-permissions.spec.ts:121–131` establishes high effort, permission text and disabled submission, fills
the model input, presses Enter, then checks the same input and existing state. An Enter handler that consumes the key
without committing the model satisfies those assertions. At this revision,
`crates/farhelm-ui/src/launch_controls.rs:167–179` displays draft text while editing and committed selection when
closed; `src/list/create_form.rs:6990–7010` maintains that distinction and closes it on blur. A committed-value
assertion after blur, with a distinct model per iteration, can distinguish acceptance from a draft that never committed.

The actual implementation does execute acceptance twice; reusing the model ID does not itself refute that. This finding
is the missing independent observable. The test detects the original effort-reset bug when acceptance runs, but can miss
a regression that prevents acceptance altogether.

Suggested bucket: **other**. Confidence: **confirmed static oracle gap; mutation/runtime behavior unexecuted**. Proposed
drop: none. Possible cover: final `TRIAGE_OUTCOMES.md:16494–16510` requires both incomplete-draft cases to be tested,
not missing acceptance evidence. Both SPECs, Planned TODOs, BUGS, queue, ledger and FILTER provide no matching
exception. Provenance: `launcher-drafts-tests-1/f2`, discovery pass 1 in the trailing review through
`fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1`. No runtime reproduction was performed.

Original finding IDs: `launcher-drafts-tests-1/f2`.

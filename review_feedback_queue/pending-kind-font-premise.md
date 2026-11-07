# Establish font readiness before testing the pending hold

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

The test intended to prove that an unresolved terminal kind blocks mounting can pass because its fonts are still loading
instead. A pass does not establish the stated reason that no socket opened during the measured interval.

## Details

Source coordinates from the original records: `e2e/grid-client/grid-attach.spec.ts:421-427`. References below use these
paths' basenames unless another path or revision is stated.

Recorder disposition: **verified**. Triage priority: **other**. Source-confirmed conditional behavior does not imply an
observed incident. Reviewer suggestions, narrower refutations and unresolved premises below remain part of the finding.

This test and the relevant lines survive unchanged at pinned tip 407a49435278844248d573083fdea6e23fd96c02. At
grid-attach.spec.ts:421 the first mount attempt syncs kind=pending. Lines 422-424 claim
`only the kind holds the mount back`, but check only the farhelmGridTerminal and Terminal globals. Lines 425-427 wait
500ms and assert no socket; lines 428-431 name host and require a direct grid mount.

The separate font prerequisite is not established before or during that absence interval. terminal.js:473 initializes
private fontSettled=false. Its only ready transition is in tryFontLoad:514-523, after the two document.fonts.load
promises resolve with nonempty results (:491-495). That work starts only when mountWhenReady calls ensureFontSettling
(:3588,550-554), which this test first reaches through sync(pending). The readiness gate also requires fontSettled
(:3631), independently of the checked globals. openPage:194-206 only installs routes and awaits goto; its static
PAGE:151-184 mounts no terminal and runs no font-readiness witness, so goto does not establish this private flag.
app.css:48-94 declares the bundled regular/bold faces and explicitly distinguishes readable chrome during a fetch from
terminal font readiness. The Playwright config:11-22 adds no setup/fixture that settles the flag. No source bound
guarantees that this new asynchronous font readiness occurs within the 500ms absence window.

A concrete competing-mechanism path: if the pending early return at terminal.js:3598 regresses and both font promises
remain pending for the measured interval, the incorrect mount still opens no socket because :3631 is false. sync(host)
then replaces the kindPending attempt through sync's pending-to-known branch (:3226-3229), and once the fonts settle,
the direct grid opens and all final assertions pass. This is a source demonstration of a missing premise/oracle, not an
observed test run or claim about failure frequency. The test must establish its other mount prerequisites before
accepting the absence interval as evidence for the pending hold. A controlled font-ready witness or a separate ready
mount followed by teardown can establish the relevant boundary; preserve the deliberate no-mount interval itself.

SPEC_impl.md, The grid client (:1825-1831 at tip), requires a potentially migrating Restart's pending attempts to mount
nothing until the kind is known; this test is meant to pin that contract. The supplied test-authoring checklist requires
`Assert the fixture premise before the action or measurement that depends on it` and
`Assert an observable that distinguishes the competing mechanisms`. The adjacent reconnect test (:450-471) separately
proves an already-mounted old terminal exists before its hold; that does not establish the missing font premise in this
first test. The issue is distinct from known g021/l4 (unrelated xterm script readiness in the real-stack integration
test) and g020/g020-c2 (producer output during a stopped-reader interval).

Suggested bucket: other, test evidence/coverage only; no established shipped user impact, security or data loss.
Confidence: confirmed missing readiness premise from source; runtime experiments prohibited and none performed.
proposed_drop: false. possible_cover: none among authorized peer records; no queue coverage checked. This resolves
discovery lead l2 under the same ID; no source premise remains outstanding for the missing oracle.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g017/l2`. The retained
append-only histories preserve all earlier claims and corrections. Reviewed source references name their own revisions;
the inclusive reviewed tip is the hash above. This review used source inspection only, with no product runtime
experiments or live-install changes.

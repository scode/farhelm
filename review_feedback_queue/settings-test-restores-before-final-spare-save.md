# Settings test restores before its final spare save settles

Reviewed commit: 5540207de26f291e29919c8611d83a859cb8fc22

## TLDR

If the prepared-checkout settings test starts with the switch off and fails during its final save, cleanup can restore
off before that outstanding save turns it on again. This can contaminate later tests on its private stack. Fresh healthy
stacks start with the switch on; the established off-state route requires an earlier fixture restoration failure.

## Details

Suggested bucket: **other**, for private test-stack configuration drift. Confidence: **confirmed conditional source
mechanism**. Adverse request ordering and frequency are unmeasured; no runtime reproduction was performed.

`e2e/tests/settings.spec.ts:50–57` captures a request only in hold-write mode. At `106–108`, the test switches to normal
mode and checks the toggle, starting a separate save of `enabled:true`. Cleanup drains only the earlier held opt-out
request (`119`), removes interception without awaiting the final request (`120`), and independently restores the old
setting (`125–130`). Checkbox interaction does not acknowledge persistence:
`crates/farhelm-ui/src/checkout_spare.rs:47–67` spawns the save. The UI's configuration fence is client-local
(`api.rs:76–78,1799–1810,1988–2008`); Playwright's restoration request does not enter it.

Helm independently spawns admitted writes (`crates/farhelm-helm/src/middleware.rs:66–74`; `lib.rs:2485–2492`). Each
unconditional update locks the connection inside its own blocking task (`checkout_config.rs:1183–1193`). This prevents
simultaneous SQL execution but does not order independently received requests. Thus restoration can commit false before
the delayed final UI save commits true.

The initial false setting is a material premise. Fresh stacks default true. The checkout fixture disables prepared
checkouts (`e2e/tests/github-checkouts.spec.ts:450–452,508–510`); a failed restoration can leave false on the private
stack reused by later projects. No healthy successful-suite route to that starting state was found. A second failure
must then interrupt this settings test while the final save remains outstanding.

Possible cover: `TRIAGE_OUTCOMES.md:14855–14879` at pinned tip `2ea2826caa0eec867c4e2bddc01badbac7bbe6c7` describes the
same cleanup-order class for another test and the checkout-root endpoint, fixed by PR #2154. It is precedent, not
coverage of this new switch test. The history-reset record concerns another write. No exact Planned, BUGS, queue or
FILTER match was found. Proposed drop: none. Drain every mutation the test issued before sending independent
restoration, including its final normal-mode save.

Provenance: trailing-review run `30f46c42-a021-475f-96a8-eac06930471b`, `g018/f1` and independent auditor `g045/f2`.

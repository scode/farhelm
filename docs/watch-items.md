# Watch items: known flake fingerprints, and what a recurrence means

NOTE: These are not TODOs. Each records a test-infrastructure failure that already happened, the mechanism that was
established for it, and the exact trigger under which a NEW occurrence becomes evidence worth investigating. Their whole
purpose is to prevent misdiagnosis: do not fold a new failure into one of these buckets unless its fingerprint matches,
and do not reflexively re-litigate a closed mechanism. Inherited from the retired MOPUP_TODO.md (2026-08-16); item
numbering from that file is preserved for cross-reference with older logs.

11. **Rotating load-class test flakes on stacked-PR CI.** The original root-cause claim here was wrong: matching libtest
    threads to the runner's four vCPUs is a useful process and memory bound, but serial fresh-process runs later
    reproduced the tmux server death. The actual trigger was closing an output-bearing tmux 3.7b control client while
    pane bytes were still queued for it. Farhelm now establishes an acknowledged client-wide `no-output` boundary from a
    separate tmux process before closing or reaping that client, including cancellation and failed-open paths. CI keeps
    the four-thread cap as a resource bound and separately runs the focused teardown scenarios against a checksummed
    tmux 3.7b build. PLAN.md's M6.5 amendment carries the detailed evidence and discriminator. Any recurrence after
    those review units is new evidence; do not classify it as oversubscription without a mechanism.
12. **Playwright flood-harness sightings.** The second-occurrence trigger fired, but it was not a recurrence of the
    original `drain socket closed before FLOOD-DONE` error. WebKit instead exhausted a fixed 45-second completion budget
    while the in-page verifier was still making correct forward progress. The whole-stream test now fails corruption
    immediately, renews a bounded stall budget when progress advances, and retains an independent hard cap for a
    producer that only limps forever. The original premature socket-close shape has still occurred once. Record and
    investigate another occurrence of either exact shape rather than folding the two mechanisms together.
13. **WebKit engine-process crashes.** A final-tip run lost both terminal and feed WebSockets when WebKit reported
    `Network process crashed`; the helm and supervisor were still healthy. The old one-project-per-engine layout had
    kept one browser alive for all 294 WebKit cases. Playwright now starts a fresh browser for each spec file while
    keeping the one-worker shared-stack contract. The failed case passed 11 fresh-process reruns and the complete
    588-case suite, whose longest remaining browser lifetime was the 21.4-minute WebKit terminal file. If this recurs
    inside a fresh per-file project, retain the trace and treat it as new evidence. Do not fold it into the
    flood-harness timeouts or a server-side disconnect unless the matching process evidence supports that mechanism.
14. **Playwright route-handler teardown failures.** A later WebKit terminal run finished the test's assertions, then
    failed because an invalidation-driven host refresh had entered `route.fetch()` just before Playwright disposed the
    page's request context. The trace showed the second GET begin before after-hooks and its JSON read fail only after
    context teardown started. Every terminal test now unregisters its page routes after the case and waits for handlers
    already in flight; the failed case passed 21 standalone fresh-process runs and the complete 588-case suite
    afterward. A recurrence is a route-lifetime problem only if the trace again shows an intercepted request crossing
    into context teardown — do not group a WebKit process crash, a server disconnect, or a request that genuinely failed
    before teardown under this heading.
15. **Review-cap residue.** The review swarms ran a hard three-pass cap, and the test-quality lens (often docs too) was
    still producing accepted findings AT the cap on every large PR: #114 (plan), #115 (proto), #117 (auth), #118
    (spawn), #119/#120 (grouping+archive), #121 (provisioning), #122 (UI provisioning), #123 (packaging) — the cap, not
    saturation, ended those reviews. Trigger: after the stack merges and before the first real release is declared
    final, run one targeted review pass (test-quality and docs lenses) over the three largest surfaces — auth,
    provisioning, packaging — and treat a pass that returns zero accepted findings as saturation reached.

# The recorded-tab scope test omits Delete's discovery and refusal

Reviewed commit: 063203912059cb23101d9a458d8821f02f1bc18f

## TLDR

The existing assertions can miss a regression where Delete skips an inaccessible scope belonging to a terminal tab.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/session_hosts.rs:858`.

session_hosts.rs:883-891 starts with a recorded main scope and observes only needs_sweep. sweep.rs:3731-3748 proves that
an already-classified tab unit fails the reap, but constructs ScopeUnits directly and bypasses teardown.rs:402-448,
where Delete discovers the tab, re-probes the manager, and refuses on enumeration failure. SPEC.md:2272-2279 requires
visible failure when a checked scope is unconfirmed. A Delete-level assertion should use an unscoped main launch, a
recorded scoped tab as the sole scope evidence, and a manager that remains unusable. The reply and retained session
would distinguish this path. Bucket: other. Assertion gap, no production failure found for a known scoped tab.
Confidence: confirmed by source and existing assertions. Possible coverage: lifecycle_unit. The in-range tip sweep test
a_recorded_tab_scope_the_manager_cannot_check_fails_the_reap explicitly documents that it covers the later reap, not
Delete's earlier enumeration. Synthesis: Stop/l2 independently identifies this same assertion gap and proposes one
combined record. Treat r5 as the canonical coverage record. At 4fc89689b56d288bbd414a0b503ea576df883aa1,
scope.rs:1072-1087 fakes successful enumeration, unlike the real branch at :1088-1109. Teardown.rs:1775-1800 covers a
recorded main scope earning re-probe and :1825-1889 covers row retention after a kill failure. These assertions leave
the tab-only enumeration refusal and row retention unpinned. Stop/l4 concerns the actual choice to skip the portable
walk after exit, a separate behavior from the r5/l2 assertion gap.

Additional source record at `4fc89689b56d288bbd414a0b503ea576df883aa1`,
`crates/farhelm-supervisor/src/service/sweep.rs:3734`.

tip sweep.rs:3734-3751 constructs ScopeUnits directly and calls the reaper. Recovery r5 adds that
session_hosts.rs:858-891 seeds a recorded main scope and checks only needs_sweep. Losing tab-specific recorded evidence
would not distinguish that premise. Tip teardown.rs:402-447 inventories the tab and main scope evidence, re-probes, and
can fail the listing before the reaper. ScopeManager's fake always returns Ok when listing (tip scope.rs:1072-1087),
unlike systemd (1088-1109). Tip teardown.rs:1775-1800 asserts recorded-main re-probe ordering and :1825-1889 asserts row
retention after a kill failure. SPEC.md:2178-2190,2272-2279 requires visible refusal and allows retry. A combined
regression assertion should provide an unscoped main, scoped tab as sole recorded evidence, and a listing refusal, then
pin the failure and retained row. Bucket: other. Confidence: source-supported assertion gap. No shipped Delete failure
established on this recorded-tab enumeration route. proposed_drop: duplicate of recovery r5. Use r5 as canonical and
carry the fake behavior, nearby-test citations, and combined oracle into it. Possible coverage: recovery r5,
lifecycle_e2e, lifecycle_unit.

Additional source record at `df6199142d6495a95b2d008400ba2686efb775c6`,
`crates/farhelm-supervisor/src/service/teardown.rs:402`.

teardown.rs:402-447 builds scope evidence from the row and tab inventory, re-probes for any recorded scope, then makes a
listing error fatal with recorded evidence. session_hosts.rs:550-579 and sweep.rs:1367-1381 confirm a scoped runtime,
placeholder with a known selection, or read-back owned tab tag can contribute a recorded unit. scope.rs:1072-1109 only
permits a listing error in the systemd mode. The Disabled and Fake paths return Ok. handlers.rs:1323-1327 gives the
specific kept-session diagnostic. No matching E2E witness was found at the pinned tip: tab_lifecycle_edges.rs:310-389
and scoped_podman.rs:192-210 expect successful Delete with a usable manager. Host_tabs.rs:1003-1086 covers the unscoped
tree. Host_tabs.rs:1169-1178,1255-1267 check guard refusal for an unreadable tab, then successful unconditional Delete.
Session_lifecycle.rs:5283-5327 pins a later launch-artifact removal error, explicitly distinguishing it from
enumeration. The #4538 exited-tab case (host_tabs.rs:373-399) observes a dead shell, live host/record, List/Attach
behavior and successful explicit Close with the manager disabled through tab_seams. It does not witness scope retirement
or the later portable sweep. SPEC.md:2178-2190,2272-2279 and SPEC_impl.md:4726-4736,4762-4772,4788-4791 separate listing
refusal from confirmation of cleanup. #4552 only renamed the direct reaper test at sweep.rs:3734-3751.
session_hosts.rs:858-891 seeds a main scope and asserts the sweep requirement, teardown.rs:1775-1800 tests main-scope
re-probe ordering, and :1825-1889 tests the later kill failure. No later in-range E2E hunk added the missing witness.
The missing combination is a demonstrably unscoped main, a scoped tab as the only recorded scope evidence, a manager
listing failure, the exact enumeration refusal, and durable/live session retention. Stop/l4 concerns a subsequent
possible sweep skip after known exit, separately. Bucket: other, test coverage. Confidence: production refusal route and
named assertions confirmed by source. No runtime observation or shipped failure on this enumeration path established.
proposed_drop: duplicate of Stop/l2 and recovery/r5. Use one merged finding, preserving these E2E boundaries. Possible
coverage: none for the exact E2E combination. The exact 0632039 parent re-probed only when entry.scope existed and
downgraded a listing error whenever available() was false. The 0632039 change uses units.has_recorded() at both
decisions. At tip, teardown.rs:258-273 returns prepare_teardown errors before finish_teardown, and handlers.rs:1280-1285
maps the error to Internal with the specific kept-session message, so the source route retains the row and live entry.
Tests under crates/**/tests have no assertion of that diagnostic. The #4552 sweep.rs diff changes the test documentation
and name only.

Additional source record at `df6199142d6495a95b2d008400ba2686efb775c6`,
`crates/farhelm-supervisor/src/service/teardown.rs:438`.

teardown.rs:354-365,402-447 settles host identity, starts with entry.scope, adds tab selections, re-probes on recorded
evidence and then lists tab and launch globs. session_hosts.rs:550-578 supplies scope evidence from a previously scoped
tab (including a settled matching Tab tag). A record with a gone/unreadable host alone only requires the portable sweep.
The UUID-shaped session and tab names matter (scope.rs:195-242. Sweep.rs:1367-1381). A failed list with recorded
evidence returns TabScopeEnumeration before finish_teardown (teardown.rs:258-273), and handlers.rs:1320-1330 reports
that the session was kept. SPEC.md:2178-2184,2272-2279 requires visible refusal when any required scope cannot be
verified and allows retry. The fake's List branch always returns Ok (scope.rs:1072-1087), even with two negative probes.
Disabled mode returns Ok(empty). Production systemd can return a listing error on Linux (scope.rs:1088-1116). Ordinary
macOS tab opens have no usable systemd scope to record, so the UUID glob lookup can fail but with no recorded scope
evidence Delete takes the portable fallback (core.rs:13940-13973. Core/host_tabs.rs:405-406). The nearby checks are
distinct: teardown.rs:1741-1801 proves re-probe-before-list and successful cleanup with a recorded main scope;
:1828-1889 proves retention after a later kill failure. Sweep.rs:3734-3751 skips Delete and calls the reaper.
Session_hosts.rs:858-909 starts with a recorded main and observes only the portable-sweep bit. The new foreign-host
witness (host_terminals.rs:1275-1310) fails earlier in discovery, uses the non-UUID s1 and an agent tag, and asserts a
foreign-tag refusal. The in-range lost-socket Close test (:4222-4297) uses an unscoped tab and tests end confirmation,
not Delete enumeration. The strengthened portable e2e case (host_tabs.rs:1003-1086, host_creates.rs:35-42) explicitly
disables scopes. The older manager-gated e2e case (tab_lifecycle_edges.rs:309-389) expects successful Delete with a
usable manager. No matching existing assertion was found in the pinned Linux/macOS source. A Delete-level fixture would
need a UUID session, unscoped agent, sole scoped tab evidence, and an injected listing error that can only be produced
after the tab is collected. Check the enumeration-specific reply and retained state before any sweep or seal. Bucket:
other, coverage gap. The source already refuses the actual error. Confidence: source-supported fixture and
production-branch distinctions. No tests run. proposed_drop: duplicate of recovery/r5 if canonical, retaining Stop/l2's
fake and test citations and this fixture inventory rather than counting separately. Possible coverage: Stop/l2 and
recovery/r5 (same coverage gap).

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery/r5`, `stop/l2`,
`lifecycle_e2e/l7`, `lifecycle_unit/f11`.

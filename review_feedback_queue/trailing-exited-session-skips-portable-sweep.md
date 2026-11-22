# Session cleanup can skip its portable sweep after exit

Reviewed commit: fc2b0b46e6211c7ca4001d05d90221f35ee5bd97

## TLDR

After a program exits and its scope disappears, Stop, Restart and Delete can skip the fallback search for its remaining
jobs.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/handlers.rs:1120`.

tip handlers.rs:867-879,945-954,1120-1132 passes a recorded scope and empty roots without forcing the walk. The shortcut
is also present at exact parent 4bd7bf366c56d7552c285e36d4d37fb3c8f54f94 handlers.rs:1121-1133. First-generation Delete
with a retained terminal and complete tab scope inventory can likewise have no live main root without forcing the walk
(tip teardown.rs:320-345,402-419,530-539. Core.rs:13541-13559). If a usable manager observes a named scope and confirms
collection, tip sweep.rs:1321-1327,1609-1626 can omit the portable search, contrary to SPEC_impl.md:4762-4772. A
recorded scope that cannot be checked is separately an error. An already-collected scope keeps the walk
(sweep.rs:2885-2913 asserts this), and no recorded launch scope sets needs_sweep from the start. Therefore this finding
needs an actual child exit plus a still-live scope, and neither explains nor duplicates recovery r6 or l8, where a
no-scope walk runs but lacks a still-identifiable root. Force the portable walk when authoritative evidence shows the
main child ended, without using host Gone as proof of that exit. Bucket: other for the source-confirmed
implementation-contract mismatch. No ordinary escaped-job consequence beyond SPEC.md:2232-2260 established. Confidence:
confirmed by source for the omission. No observed survivor. Possible coverage: lifecycle_unit, lifecycle_e2e.
Neighboring Restart branch in recovery.

Additional source record at `063203912059cb23101d9a458d8821f02f1bc18f`,
`crates/farhelm-supervisor/src/service/core.rs:12585`.

core.rs:12494-12509 and :12571-12598 take the ended-agent path with ScopeUnits::recorded(entry.scope.clone()) and no
explicit require_sweep, and normally no outstanding child roots. sweep.rs:1321-1327 sets needs_sweep to false for a
known unit. At :1609-1626 a scope still live at inspection and confirmed ended afterwards permits early return,
including with an empty root set. A scope already gone at entry sets needs_sweep instead, and an unconfirmed scope
causes Refuse at :1629-1647. An unscoped Restart always walks and is therefore outside this finding.
SPEC_impl.md:4762-4772 explicitly requires the portable sweep for Restart after agent exit. Mark the confirmed-exit
branch as requiring the portable walk and extend existing scope assertions to cover the live-then-collected case. This
is distinct from r6 and Stop/l8, which concern losing a live pinned child even when the walk runs. Bucket: other for a
source-proven skipped required walk. No identifiable ordinary survivor beyond the accepted scoped containment contract
established. Confidence: confirmed by source, no observed runtime behavior. proposed_drop: merge into Stop/l4 if its
canonical record explicitly includes Restart and preserves these scope and consequence limits. Possible coverage:
Stop/l4, lifecycle_unit and lifecycle_e2e. The parent fc2b0b46e6211c7ca4001d05d90221f35ee5bd97 core.rs:12571-12598 has
the same Restart branch, so this is a neighboring pre-existing mismatch, not introduced by terminal_missing. For Delete,
Stop/l4 limits the corresponding shortcut to complete terminal/tab discovery. A missing terminal or later generation
forces sweeping at core.rs:13544-13557 and teardown.rs:402-419. Shared fix boundary with Stop/l4: callers carry
authoritative child-exit evidence into ScopeUnits::require_sweep before the common reap gate, so a successful
live-then-collected scope cannot suppress the required walk. The reaper should continue to accept scope-only cleanup
where containment and live-target evidence justify it. Gone alone is terminal availability, not child-exit evidence.
Stop/l9 and l10 must validate live or unresolved child identity before accepting ended-only consent. That guard is
separate from carrying ownership evidence into an already-authorized cleanup. Count a single user-facing scoped-walk
finding with Stop/l4, including Restart as a checked route.

Additional source record at `df6199142d6495a95b2d008400ba2686efb775c6`,
`crates/farhelm-supervisor/src/service/teardown.rs:402`.

for a generation-zero host session with a scoped main and all tabs recorded scoped, core.rs:13532-13559 includes the
exited tab and marks the inventory complete. session_hosts.rs:243-260 keeps held tabs as runtimes, :510-521 and
hosts/terminal.rs:759-789 produce no sweep root for the exited child, and session_hosts.rs:554-579 carries the recorded
tab scope without requiring a sweep because its child ended. teardown.rs:402-447 and :530-539 add the scopes and any
still-live roots, with no exit-based sweep flag. If the scoped tab's cgroup is still live with a descendant, enumeration
succeeds, all named scopes confirm live-then-collected and no carried root survives, sweep.rs:1610-1629 returns before
the portable sweep. SPEC_impl.md:4762-4772 explicitly requires the sweep for Delete after an already-exited tab.
host_tabs.rs:373-399 pins the death-to-reap window and explicit Close using disabled scopes; :552-585 pins automatic
reap with the same disabled setup. session_lifecycle.rs:4184-4223 witnesses the opposite expectation for Stop of a live
scoped main. Existing assertions do not discriminate this successful-scope, exited-tab Delete route. Bucket: other for
direct implementation-contract mismatch. The recorded route confirms the omitted walk, and the source does not establish
ordinary user work surviving outside accepted containment. Confidence: confirmed by source for the conditional route. No
runtime failure observed. proposed_drop: merge this exited-tab variant into Stop/l4 if it is canonical for the portable
sweep mismatch. This is separate from l7, where enumeration fails and the reap is never entered. Possible coverage: none
for the exact ended-tab successful-scope condition. This requires a scoped main as well as a recorded scoped tab: with
an unscoped main, sweep.rs:1321-1327 sets needs_sweep immediately. A scope already gone at the first check also forces
fallback (sweep.rs:1609-1615). The exited main unit check in handlers.rs:4647-4665 models a failed kill, so it cannot
observe skipping the sweep after successful scope confirmation. Later in-range changes to the handler are test-only and
#4552 alters only the direct reaper test name and comment. The #4538 unscoped setup comes via host_tabs.rs:35-40 and
host_creates.rs:35-42.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `stop/l4`, `recovery/r11`,
`lifecycle_e2e/l8`.

# Historical-base deflake runs lose their selected revision's required tmux check

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Source-confirmed review record: starting the updated sweep against an older --base can run that revision's tests without
the pinned tmux prerequisite and required substrate check that its sweep previously enforced. The older recorder prints
a mismatch warning, but the updated driver lets the battery run on a different tmux and can report it finished; this
weakens the evidence for historical-base flake investigations. Source route: current deflake/bin/deflake:1624 accepts
--base REV without limiting its age; :1454-1457 launches the current checkout's driver, :1191-1218 checks the selected
base into a separate workspace, and :925 invokes that workspace's recorder. The changed :1236 merely runs machine tmux
-V instead of building the selected workspace's pinned tmux, and :926-927 no longer passes --tmux required. At the
immediate prior base d751ff16^, record-test-run.py:230 defaults an omitted --tmux to warn; :1193-1227 compares against
the selected checkout's pin and .ci-tmux/tmux, while :1981-1992 only refuses mismatches for required mode. Thus with a
clean sweep workspace and any runnable non-pinned machine tmux, new prepare succeeds, the old recorder detects the
missing pinned binary/version or hash mismatch, prints it, then continues rather than refusing. Parent
deflake/bin/deflake:1231-1253 instead built and prepended that selected revision's pinned tool, and its phase/recorder
code supplied required for the affected Rust/browser phases (the assigned patch shows the removed field and arguments).
The pinned range begins with the same required substrate model, so this regression is introduced by the assi

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g094/l2`.

### g094/l2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 19 (finding, open):

Reviewed commit: d751ff1633bb479c7bf53920f2e4ea4ba932a1c2. TLDR: starting the updated sweep against an older --base can
run that revision's tests without the pinned tmux prerequisite and required substrate check that its sweep previously
enforced. The older recorder prints a mismatch warning, but the updated driver lets the battery run on a different tmux
and can report it finished; this weakens the evidence for historical-base flake investigations. Source route: current
deflake/bin/deflake:1624 accepts --base REV without limiting its age; :1454-1457 launches the current checkout's driver,
:1191-1218 checks the selected base into a separate workspace, and :925 invokes that workspace's recorder. The changed
:1236 merely runs machine tmux -V instead of building the selected workspace's pinned tmux, and :926-927 no longer
passes --tmux required. At the immediate prior base d751ff16^, record-test-run.py:230 defaults an omitted --tmux to
warn; :1193-1227 compares against the selected checkout's pin and .ci-tmux/tmux, while :1981-1992 only refuses
mismatches for required mode. Thus with a clean sweep workspace and any runnable non-pinned machine tmux, new prepare
succeeds, the old recorder detects the missing pinned binary/version or hash mismatch, prints it, then continues rather
than refusing. Parent deflake/bin/deflake:1231-1253 instead built and prepended that selected revision's pinned tool,
and its phase/recorder code supplied required for the affected Rust/browser phases (the assigned patch shows the removed
field and arguments). The pinned range begins with the same required substrate model, so this regression is introduced
by the assigned commit rather than inherited from the range base. Contract: the public CLI and prepare documentation
promise the sweep runs against a selected jj revision; prerequisite setup and the recorder contract come from that
revision. Suggested bucket: other (maintainer tooling/evidence correctness; no catastrophic consequence established).
Confidence: confirmed conditional source route, no runtime observation. proposed_drop: false. possible_cover: none; no
prohibited coverage search. Could-be-highest: no established security, user-work-loss or catastrophic tooling route.
Caveats: default runs with a base at or after this commit have no old tmux gate and are not affected; cached pinned
artifacts may mask the missing build; a mismatched test may fail visibly, and the older recorder does emit a warning, so
no claim of universally green tests or hidden diagnostics. The defect is lost required-substrate enforcement for
supported historical base selection, not a claim that current host terminals require tmux. A fix can align
preparation/phase policy with the selected revision or explicitly reject incompatible historical bases before a sweep
starts. Lead l2 is resolved to a finding after checking launcher ownership, selected recorder defaults, refusal logic
and preparation ordering. No reproduction or runtime test performed.

Endpoint check during discovery: git log d751ff16..00de0b6c -- deflake/bin/deflake contains no later change. Pinned tip
00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains recorder_argv:924-930 without --tmux, the machine probe at :1236,
current-driver launch at :1454-1457 and unrestricted --base option at :1624. The historical-base evidence-policy
mismatch therefore remains at the pinned tip; it is not merely an intermediate state. Parent phase source was also read
at d751ff16^:591-615 and :925-931 and range-base :927-933, confirming the former required flag for
Rust/browser/doctest/desktop-smoke phases. No execution.

Returned proposal (claims-062, event 19): **retain-verified**, suggested bucket **other**.

Selecting an older source revision retains the new controller but invokes the old recorder. The controller no longer
builds that revision's pinned tmux or passes its required mode, so the old recorder warns and continues instead of
enforcing the selected revision's former gate. This weakens historical-base test evidence under the stated conditions.

Original stronger consequence: Initial lead asked whether an older supported base could no longer run. The established
consequence is weaker evidence: missing pin preparation and required enforcement, while a mismatch warning remains
visible and tests may fail anyway. Default bases at/after the removal are unaffected; cached pinned artifacts may mask
it. No security, user-owned-work loss, catastrophic tooling harm or universal green result is established.

Unresolved premise or limitation: None for the conditional loss of required enforcement. No explicit promise to support
every arbitrarily old base was found; the unrestricted CLI and immediate-parent example make retention appropriate
rather than inventing a cutoff. Runtime frequency and green historical sweeps were not observed.

Proposed basis: Retain as other for the verified conditional source-policy mismatch. Preserve the historical-support
ambiguity and runtime caveats in the queued record.

Existing parent disposition (event 19): verified. Parent explicitly accepted named retain-verified proposal from
claims-062 after inspecting it. Selecting an older source revision retains the new controller but invokes the old
recorder. The controller no longer builds that revision's pinned tmux or passes its required mode, so the old recorder
warns and continues instead of enforcing the selected revision's former gate. This weakens historical-base test evidence
under the stated conditions. Retain as other for the verified conditional source-policy mismatch. Preserve the
historical-support ambiguity and runtime caveats in the queued record. Checked source/results:
["d751ff1633bb479c7bf53920f2e4ea4ba932a1c2:deflake/bin/deflake:1454-1457 launches this checkout's controller, :1191-1218
checks out self.base, :924-930 invokes that workspace's recorder without --tmux, :1236 only probes machine tmux, and
:1624 accepts --base as a jj revision without an age
limit.","d751ff1633bb479c7bf53920f2e4ea4ba932a1c2^:scripts/record-test-run.py:230 defaults omitted --tmux to warn;
:1193-1227 compares machine version/hash with selected checkout pin/.ci-tmux/tmux; :1981-1992 emits mismatch diagnostics
but refuses only required mode.","The parent controller's phases at :591-615 require tmux for the affected
Rust/browser/doctest/desktop-smoke phases; :925-931 passes that mode; :1231-1253 builds and prepends the selected
revision's pinned tmux. The exclusive range base has the same older prerequisite arrangement at
:1231-1255.","00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db retains the new recorder command, machine probe,
current-controller launch and unrestricted --base at the same lines. No later in-range deflake change repairs this path.
No runtime reproduction."] Current coverage/results: ["SPEC.md and SPEC_impl.md searches found no explicit
historical-base compatibility cutoff or acceptance of weakening an older selected revision's recorder policy.
SPEC_impl.md:8216-8229 single-user/one-command assumptions do not settle cross-revision prerequisite behavior.","TODO.md
Planned is empty. plans/queue searches for historical/older base, selected recorder, deflake and pinned tmux found no
pending repair. The landed tmux-drop-from-tooling plan:37-55 intentionally removes current pinning and mode, but does
not explicitly reject old bases or address their retained required-mode contract. That broad removal intent is not an
exact historical-base acceptance.","BUGS.md has no deflake entry. INDEX.md and active TRIAGE_OUTCOMES.md searches found
only unrelated deflake exclusion/parent-directory items; none matches this cross-revision trigger and evidence-policy
consequence.","FILTER.md does not match: selecting an older source base is not an installed-release upgrade plus a rare
condition, and lost required-substrate enforcement is not wholly a rare transient diagnostic or safe retry failure."]
Exact cited cover: [] Original stronger consequence and missing premise remain preserved: "Initial lead asked whether an
older supported base could no longer run. The established consequence is weaker evidence: missing pin preparation and
required enforcement, while a mismatch warning remains visible and tests may fail anyway. Default bases at/after the
removal are unaffected; cached pinned artifacts may mask it. No security, user-owned-work loss, catastrophic tooling
harm or universal green result is established."; "None for the conditional loss of required enforcement. No explicit
promise to support every arbitrarily old base was found; the unrestricted CLI and immediate-parent example make
retention appropriate rather than inventing a cutoff. Runtime frequency and green historical sweeps were not observed.".

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g094 record](records/review-through-00de0b6c/groups/g094.json). Record ID: `l2`.

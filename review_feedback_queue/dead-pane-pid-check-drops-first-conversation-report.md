# Dead-pane PID checking drops the first conversation report

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

If the first conversation report arrives after the agent exits on tmux 3.8, Farhelm can permanently lose ordinary
Restart for that session.

## Details

Suggested bucket: **high**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The report reader refuses a dead pane lacking its PID before capture (service/core.rs:16525–16535;
service/report_files.rs:248–267,322). The shared path affects first-class Claude and Codex before vendor dispatch. A
report never admitted cannot supply the ordinary captured Resume target, leaving manual named recovery as a separate,
more burdensome option. The accepted older recorded-target derivative does not settle losing the first offer; Near-term
TODO is not Planned coverage. Parent priority is high for permanent loss of an ordinary feature, preserving g012's lower
reviewer suggestion and uncertainty about prevalence. No already-valid offer or transcript loss is claimed; no vendor
turn or tmux experiment ran.

Source locations:
`TODO.md:35-42 in 2c6056a7c88f41c16a21bc425587f5c257fb3316; pinned behavior at crates/farhelm-supervisor/src/service/core.rs:16525-16535 and service/report_files.rs:248-267,322`.
Historical revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Synthesis revision: suggested bucket other, replacing the initial high suggestion. The established defect is permanent
failure to capture the waiting conversation report. With no prior identity this means no automatic Resume; with an older
identity the stale-old-target derivative is explicitly accepted by SPEC.md:2442-2449 and is not a separate finding.
Initial high wording equated missing automatic Resume with material/severe user impact without establishing that
recovery burden. SPEC.md:1383-1386 explicitly allows an agent launch's Restart with to take a user-named conversation
even without capture, so the source does not establish that the conversation itself is destroyed or inaccessible outside
automatic capture. No common trigger rate, severe recovery burden, lost vendor record, destructive wrong-target action,
or security route was established. This is a consequence-based bucket revision, not a filter/drop and not an inference
that the tracking cause is accepted. Missing automatic Resume remains a durable correctness defect, rather than a
transient diagnostic. Confidence confirmed conditional source mechanism; timing prevalence and actual vendor/reporting
behavior have not been independently authenticated. The shared admission doorway affects Claude Code and Codex as well
as other kinds (SPEC_impl.md:3287-3298; report_files.rs:304-337); FILTER.md Rare edge cases in harnesses without
first-class support therefore cannot exclude the whole finding. SPEC_impl.md:3289-3290 known-gap wording remains
ambiguous acceptance; TODO.md:35-42 is Near term, not Planned. proposed_drop: deduplicate into the same cause
independently retained by a009 and g097/g100/g180/g181, as reported by parent, preserving the no-prior-capture
consequence and first-class scope. This is not a final substantive exclusion or a highest-candidate withdrawal.
possible_cover: only the exact stale-old-recorded-target derivative has explicit acceptance in SPEC.md:2442-2449; no
unambiguous whole-cause coverage established. Parent still owns any potentially-highest exclusion check. Additional
checks this turn: read SPEC.md:1348-1397,2435-2449 and SPEC_impl.md:3285-3294; source confirms the manual named-resume
contract and distinguishes the never-learned defect. Prior source checks established missing-pid Conflict,
settled-report deletion and a supervisor-downtime producer. Missing premises for stronger priority: a supported sequence
and evidence showing material recovery failure/burden or irreversible loss of user work, distinct from the explicitly
accepted stale-old-target consequence. This source-only bounded synthesis cannot measure frequency or authenticate
vendor persistence/recovery; narrowly owned runtime evidence or a complete stronger source route would settle those
premises.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g012/f1`, `g097/f2`, `g100/f3`, `g180/f1`,
`g181/f1`, `a009/f1`, `a018/g180-f1`. These are original reviewer identities for this concern, not additional bugs. Full
revisions, withdrawals and independent verdicts remain in the private review evidence. No fix or triage decision was
executed.

# A dead child without a pinned start token leaves report origin uncertain

Reviewed commit: 407a49435278844248d573083fdea6e23fd96c02

## TLDR

A report whose recorded ancestry happens to contain the reused process number of a host's already-dead child may be
accepted as belonging to the current run. If it selects another run's conversation, a later Restart could resume and
append user work there. This original potential-highest route is unverified after source investigation; the prior
complete exclusion was ambiguous and is not retained.

## Details

Source coordinates from the original records:
`crates/farhelm-supervisor/src/hosts/terminal.rs:1783-1785,885-895;procs.rs:557;service/report_files.rs:321-335`.
References below use these paths' basenames unless another path or revision is stated.

Recorder disposition: **unverified-after-investigation**. Triage priority: **highest**. Source-confirmed conditional
behavior does not imply an observed incident. Reviewer suggestions, narrower refutations and unresolved premises below
remain part of the finding.

Supported missing-token producer confirmed: ordinary Info.child_exit -> Child::Reaped(pin_child2566-2567,2576) ->
accepted adoption with child_start=None(1783-1785). anchor885-895 keeps that pid after exit. anchor_chain557 compares
the pid but only compares RecordedLink.start when anchor.start is Some. Ordinary CLI hook.rs338-356 records its own live
process ancestry and writes the credential's session directory; HookReport142-161 carries no originating generation.
This is not a hostile/handwritten-chain allegation. Another session's ordinary hook writes its own directory, so the
investigated remainder is stale SAME-session run evidence or a supported inherited same-session emitter, not an
established cross-session credential route.

Source checks did not find an independent origin fence. report_files321-335 anchors first, then supplies the row's
CURRENT generation; core17998-18008 and18597-18607 validate current row/entry consistency only. A normal Claude/Muse
report has the supported reporter/trampoline shape(positional.rs40-75). First-emitter admission accepts when
pin=None(vendor/positional95-121), relaunch clears that pin(store4483-4488), and final CAS5460-5465 checks current
generation/binding/pin, not the report's original launch. The potential admission predicate is therefore real.

Report lifetime narrowed by actual production code: Restart12904 waits and drains existing files before deciding, and
ordinary old LIVE reporters are swept. But reports published after an empty drain listing explicitly wait for the next
pass(report_files436-438,480-485); publication-gap entries do not remove still-durable directories(158-165,348-352),
row/entry generation mismatch243-244 retries, and528-533 restores the file. No blanket assertion that every
earlier-launch report survives is made; these are the concrete file-retention boundaries checked.

Exact remaining missing premise: a complete ordinary shipped sequence in which (1) such a same-session report is still
pending across relaunch, (2) the replacement child's pid equals a relevant recorded ancestor pid, and (3) the
replacement is already reaped when adopted, giving no start token, before any newer selecting report or settled refusal
supersedes that file. Source verifies the three mechanisms individually and the acceptance if they coincide, but does
not establish the actual OS pid-allocation/scheduling sequence or an ordinary emitter occurrence combining them. No
shipped control selects an arbitrary pid. Read-only source cannot measure that timing, and runtime/process experiments
are forbidden by this charter. An isolated owned-process reproduction of that sequence, or an independently reviewed
deterministic process/Info/report fixture showing admission of an ordinary earlier-run chain under a reused dead root,
would settle the remaining route. No uninvestigated code premise is hidden behind this uncertainty.

SPEC.md Durability and resume2909-2924 promises the session's own reported conversation and identifies
wrong-conversation append as the harm. SPEC_impl.md4524-4528 says anchoring ties a report to its launch;4749-4755 says
an earlier-launch report fails anchor. Its4990-5013 short bounded process-number window exception does not by itself
settle this stored historical pid-only anchor: this record does not propose a blanket exception or reinterpret every
retained report as a bounded immediate check. SPEC.md2864-2871 accepts brief tracking lag/continuation with the
conversation Restart chose, but that does not establish acceptance of arbitrary wrong-origin report admission; no drop
proposed on that text.

Suggested bucket: retain potential highest per parent/ambiguous independent consultation, for wrong Resume and appended
user work; no established consequence lowered. Confidence: unverified after investigation for the complete ordinary
wrong-run collision sequence; supported missing-token producer, ordinary emitter, file retention, optional comparison
and absence of an independent original-generation gate are source-confirmed. proposed_drop:false for this remainder.
possible_cover:none in refreshed authorized peer records; no coverage-source search. Narrow original refutations remain
exact: the rename preserves fields/matching, dead report anchors never supply live kill roots, host live/root callers
revalidate pinned identity, and Delete's pinned_child_root is separate. The distinct original reused-pid SIGNAL
consequence from the shared-type move remains code-refuted on those checks, not resurrected by this report-origin
uncertainty. No blanket rename or pre-base exclusion, no measured incident, no runtime experiment. Independent exclusion
remains parent's responsibility.

Review provenance: trailing-review run `7c7c7b55-12fc-4573-b10e-01d690187f50`; original records `g028/g028-l2`. The
retained append-only histories preserve all earlier claims and corrections. Reviewed source references name their own
revisions; the inclusive reviewed tip is the hash above. This review used source inspection only, with no product
runtime experiments or live-install changes.

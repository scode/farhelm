# Pre-existing excerpt contamination; in-range implementation spec repeats a false blank-screen proof

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: Considered whether a failed Restart could quote the previous run’s terminal output after this
patch removed the old-terminal caveat. Source refutes that path for terminal hosts.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g009/l1`, `g013/f2`, `g100/c1`, `g110/c2`, `g117/c4`, `g161/c4`, `g165/c3`.

### g009/l1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 8 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Considered whether a failed restart could misdiagnose
the new launch by quoting the previous run's final screen. The source excludes that route. Details:
SPEC_impl.md:2752-2765 promises an excerpt of at most five nonblank final-screen lines and 500 escaped UTF-8 bytes, from
the current host only. service/launch_artifacts.rs:59-80 gates quoting on HostEvidence::Owned { dead: true }; :89-132
implements the line and byte bounds. service/core/host_restart.rs:418-488 obtains the old host's history seed.
terminal-emulator/src/snapshot/mod.rs:541-570 selects v1::encode_history; snapshot/v1/mod.rs:246-268 delegates to
paint_history. snapshot/v1/paint.rs:404-436 truncates the source rows to primary history, adds blank rows for the entire
new screen, and restores fresh cursor state. Thus old history survives only above a blank screen, as the contract says.
Suggested bucket: other for the originally suspected misdiagnosis, but no defect established. Could-be-highest: no; no
established security or loss-of-work route, and retained same-session history is already accessible. Confidence:
confirmed source refutation. proposed_drop: refuted by the history-only encoder, not an exception accepted only by
specification. possible_cover: none. This closes lead l1. No execution or reproduction performed. Synthesis turn 3: git
diff 1ed482dde220452e424973faf38a5e1318986333 f560058da19e02716e6f943883a9508b447aff4e over
terminal-emulator/src/snapshot, service/launch_artifacts.rs and service/core/host_restart.rs is empty. The next assigned
commit therefore does not change the seed, blank-screen or final-screen-excerpt premises. Original consequence and
source-refuted disposition are unchanged. Discovery input from g013/f2:11 challenges the earlier refutation:
primary-grid growth can promote inherited scrollback to the visible screen before a still-unconsumed launch spec fails.
Reopened under original ID before source investigation. The original misdiagnosis consequence remains; the initial
blank-seed proof alone does not settle later resize behavior. Need to verify resize admission, grid promotion,
final-screen quoting and introduction within the full pinned range. No new runtime observation, bucket change or
accepted drop. Source result supersedes the earlier candidate refutation: at reviewed commit
1ed482dde220452e424973faf38a5e1318986333, vendor/alacritty_terminal/src/grid/resize.rs:25-28 calls grow_lines; :46-71
pulls inherited history into visible rows on height increase. term/mod.rs:756-758 enables this on the primary grid.
host_launch.rs:508-518 supplies the Restart snapshot; :579-645 awaits host start/adoption, then tracks the still-pending
spec and returns. host_restart.rs:294-305 confirms the socket and :363-388 publishes the terminal without awaiting shim
consumption. handlers.rs:2050-2083 resolves the attached terminal and admits a valid viewer resize;
hosts/terminal.rs:2344-2353 queues it with no launch-readiness condition, :3358-3362 routes its resize record to
on_resized, and :3872-3885/:4063-4070 apply it to the emulator. status.rs:630-652 calls append_launch_excerpt for a dead
host with an unconsumed spec and no sentinel. screen.rs:50-96 includes all visible rows; launch_artifacts.rs:97-105
discards blank rows before selecting the last five nonblank rows. Consequently a height increase during ordinary
login-shell initialization can expose inherited prior-run history, and a subsequent shell failure can quote those old
messages as this failed launch output. Processed final-exit evidence establishes ordering, not cell provenance. Current
TLDR: a failed restarted session can durably quote an unrelated previous-run message as its last terminal output.
Suggested bucket remains other (misleading diagnostic); could-be-highest: no established security or work-loss route.
Confidence: confirmed conditional source/control-flow route, not runtime observed. Preconditions: inherited history,
growth after new-host publication and before classification, shell failure before shim consumes its spec, and at least
one promoted row not overwritten. proposed_drop: false; withdraw the earlier source-refuted proposal, retaining its
exact historical rationale above. Contract defect in assigned patch: SPEC_impl.md:2759-2763 says everything on the
screen is this launch output and a blank initial screen means no quote reaches the previous run. This unqualified
provenance statement is false after resize. Runtime attribution: the excluded base already has the same host quote and
blank-seed rationale at launch_artifacts.rs:52-101. Do not attribute introduction of the runtime route to this
documentation relocation. possible_cover: provided interface records g013/f2 and g100/c1 identify the same source route;
parent should reconcile as one cause. No prohibited coverage source searched. No open source premise remains for the
conditional route; full-range wording attribution will be refined separately under this ID. Whole-range attribution
refinement: excluded-base SPEC_impl.md:3260-3289 documents the tmux capture fence and its acknowledged contamination
residual, but does not state this new host-only no-previous-run guarantee. The assigned 1ed482d patch adds that
host-only paragraph at :2756-2765 while removing the old capture description. Thus the assigned documentation newly
presents an invalid unconditional proof even though the runtime contamination route and the equivalent helper-doc
rationale were already present at the range base. The base-to-reviewed diff for grid/resize.rs is empty; snapshot paint
changes remove only sgr_setting and its re-export, leaving paint_history unchanged. This record reports the incorrect
authoritative host-attribution contract, not a claim that this commit first introduced runtime misattribution.
emulator.rs:224-226 independently confirms resize calls the vendor term resize. Scope and bucket remain other; no new
effect or highest route is alleged. Full-range wording attribution is now settled.

Returned proposal (claims-004, event 8): **retain-verified**, suggested bucket **other**.

The initial blank screen does not prove all later visible cells came from the new child. Height growth can promote
inherited history before failure classification. Retain the newly authoritative unconditional provenance explanation as
an in-range documentation defect; the runtime mechanism predates the range.

Original stronger consequence: A failed restarted session can durably quote a previous-run message as its last terminal
output. This is misleading diagnosis only, with no established security or work-loss route. Initial source-refuted
proposal is withdrawn, not accepted.

Unresolved premise or limitation: For an observed misdiagnosis: inherited history, growth after replacement publication
but before classification, shell failure before shim consumption, and at least one promoted nonblank row surviving
overwrite. Source admits these conditions; no reproduction establishes their actual occurrence.

Proposed basis: Retain other for the in-range documentation contradiction. Do not present the pre-range runtime route as
newly introduced, or claim the whole terminal feature is broken.

Existing parent disposition (event 8): duplicate. Parent inspected both complete histories during primary synthesis and
read focused004 and006 exact source/current-cover checks. Both latest claims retain the newly authored1ed482 SPEC_impl
blank-screen provenance argument, refuted by ordinary height growth promoting inherited history before failure
classification. Runtime mechanism and SPEC.md promise predate the exclusive base; the new false implementation proof
does not. Same underlying documentation defect/location and concrete counterexample, so one retained queue item
preserves both original IDs and all earlier proposals. This dedup does not exclude either concern, reinstate the
superseded blank-screen refutation, infer a new runtime regression or lower an established higher consequence.
Existing013/f2 verified member anchors the same canonical group; diagnostic-filter ambiguity requires retaining the
documentation remainder.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g009 record](records/review-through-00de0b6c/groups/g009.json). Record ID: `l1`.

### g013/f2

Confidence: Source-confirmed by an existing parent decision; not runtime reproduced.

Latest reviewer record, event 16 (finding, open):

Reviewed commit: f96dd4363d97dc7e07dea5b8947c96be6bf912e8. TLDR: Considered whether a failed Restart could quote the
previous run’s terminal output after this patch removed the old-terminal caveat. Source refutes that path for terminal
hosts. Details: SPEC.md:2892–2897 removes the exception for a tmux terminal retaining an earlier run’s screen. Restart
still preserves history, but crates/farhelm-supervisor/src/service/launch_artifacts.rs:59–75 quotes only
host.screen_tail after owned-dead evidence. hosts/terminal.rs:2229–2239 and
farhelm-terminal-emulator/src/emulator.rs:265–275 show that this reads the visible screen rather than history.
farhelm-terminal-emulator/src/snapshot/mod.rs:541–569 delegates Restart’s seed to encode_history, whose implementation
in snapshot/v1/mod.rs:242–268 paints primary history above a blank screen, not the prior visible frame.
SPEC_impl.md:2657–2668 gives the same contract, including waiting for drained output. This is source analysis only.
Suggested bucket: other if it existed (misleading launch diagnosis). Confidence: confirmed refutation of the considered
route. Could-be-highest: no established security or loss-of-work route; quoting a prior error alone would misdiagnose a
failed launch. proposed_drop: Refuted by the visible-screen-only quote and history-only Restart seed at the cited source
locations. possible_cover: none. Caveat: this does not claim terminal-host recovery or arbitrary emulator behavior was
independently reviewed; the assigned change only removes the retired tmux exception. Synthesis lead: the existing
refutation establishes only that the initial seed has a blank visible screen. The caller may resize that seeded emulator
before launch classification. emulator.rs:224–226 delegates resize to alacritty; paint.rs:401–424 confirms Restart
history is still in the primary grid. Need check whether a grow/reflow can move retained history into the visible
screen, and whether an ordinary shipped attach/resize can reach that state before append_launch_excerpt. If so, the
earlier visible-screen-only argument is insufficient. This is an unresolved source check, not observed behavior; retain
original consequence and other-bucket assessment pending investigation. Source result:
vendor/alacritty_terminal/src/grid/resize.rs:25–28 calls grow_lines on increased height; 46–71 pulls up to lines_added
retained history rows into the visible grid and shifts the cursor. term/mod.rs:756–758 enables that primary-grid resize,
reached by emulator.rs:224–226. The blank seed therefore does not remain a proof of single-launch screen provenance
after growth. status.rs:622–632 requests append_launch_excerpt for the unconsumed-spec/no-sentinel failure shape.
Remaining check: ordinary attach resize reaches the new host before that classification, and the excerpt reader includes
the pulled rows. Earlier refutation is invalid as a general claim; not yet claiming observed runtime behavior. Synthesis
conclusion (supersedes the candidate refutation): the caller check establishes a source route. handlers.rs:1881–1887
resizes during attach and 2042–2054 accepts viewer resizes for the attached terminal; hosts/terminal.rs:2341–2353 queues
any valid size while the runtime exists, and 3340–3344 routes the resulting host resize record to on_resized, which
reaches resize_emulator (4040–4050) and alacritty. A terminal host’s child can still be in login-shell initialization
when Restart publishes its terminal; no launch-readiness check in these resize handlers requires that the shim consumed
the launch spec. If the user increases height while that shell is still starting, grow_lines pulls prior-run history
into the visible screen. If that shell then exits before consuming the spec, status.rs:622–632 invokes the excerpt path;
screen.rs:50–94 includes every visible row, and launch_artifacts.rs:96–102 discards blank rows and selects the last five
nonblank ones, so blank rows below the pulled history do not exclude it. No source branch distinguishes those
old-history rows from current-run output. Current TLDR: after Restart, enlarging the terminal while the login shell is
still starting can cause a subsequent startup error to quote the previous run’s output as the failed launch’s last
output. A quiet shell failure can therefore show an unrelated old message as its explanation. Contract:
SPEC.md:2890–2893 retains ‘Farhelm omits the excerpt when it cannot safely attribute the text to that launch’ while
deleting the old-terminal contamination exception. SPEC_impl.md:2657–2668 and launch_artifacts.rs:44–56 assert that a
fresh blank screen prevents any quote from reaching back into the prior run; that argument misses the subsequent growth
of a history-seeded terminal. Suggested remedy: retain launch-output provenance across seed/history promotion, or omit
an excerpt once resizing could have exposed inherited history. Merely reading the visible screen is insufficient.
Reviewed commit remains f96dd4363d97dc7e07dea5b8947c96be6bf912e8. Current confidence: confirmed by source/control-flow
analysis, not observed execution; requires existing retained history, a height increase after the new terminal is
published and before error classification, and a login shell that exits without consuming the spec or overwriting all
pulled rows. The precise timing is conditional but no unsupported operation is required. Current suggested bucket: other
(misleading durable launch diagnostic). Could-be-highest: no established security or loss-of-work route. proposed_drop:
false; withdraw the earlier source refutation under this same ID. possible_cover: none; no forbidden coverage searches.
This is reconciliation of f2, not a separate ID or independent new scope. Further source checking of publication below
may refine the stated timing. Publication check settled: host_launch.rs:508–518 supplies the inherited-history snapshot
in ChildSpec, 579–625 waits only for the terminal host start and adoption, and 639–645 returns that host while tracking
the still-pending launch spec. host_restart.rs:294–305 confirms the socket and 363–388 publishes it and replies; none
awaits the login shell consuming the launch spec. terminal.rs:3854–3866 applies the resize record before later exit
classification. This establishes the stated source route without a special test seam. The previous sentence about a
further publication check is now settled, with no open premise beyond the stated ordinary timing and shell-start failure
conditions. Source-only, not reproduced. The affected diff removes the retired-terminal contamination exception; this
record concerns the remaining attribution promise, not an independent review of unrelated emulator behavior. Full-range
attribution check requested by parent: retain the established conditional resize/history-promotion/excerpt route and all
prior consequence fields, but do not yet attribute its introduction to f96dd436. The assigned removal of a tmux-only
exception may leave an already-existing host promise unchanged. Comparing exclusive base
6e423e30068b62c6cfa276c6d720ee35bb70387a for host seed, resize, excerpt, attribution promise and blank-screen rationale,
then selected history for any new false proof/documentation statement. No new ID and no disposition accepted.
Exclusive-base comparison result so far: SPEC.md at 6e423e3 already has the safe-attribution promise at 2852–2854 and
says a new terminal never shows a prior-run screen at 2855–2856; the exception removed by the assigned patch is
explicitly for old terminals. The base host append_launch_excerpt at launch_artifacts.rs:80–94 already quotes
host.screen_tail through the shared reducer without a history-provenance fence. SPEC_impl.md at base 2440–2445 already
seeds old history above a blank screen. Thus neither the host excerpt path nor the host safe-attribution promise was
introduced by the assigned patch. Remaining checks: verify base grow/resize plumbing and determine whether the in-range
rewritten blank-screen explanation is newly false in meaning or merely repeats the same base assertion. Full-range
attribution resolved: the conditional runtime route predates exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a. At
that base, snapshot/v1/paint.rs:401–437 already seeds old history above blank rows; vendor/grid/resize.rs:41–72 already
promotes history on growth; handlers.rs:1912–1918 and 2229–2235 resize through terminals.rs:271–283 to the host;
launch_artifacts.rs:80–94 already quotes the visible host screen. The base safe-attribution promise and host-only
assertion are already at SPEC.md:2852–2857. The base launch_artifacts.rs:52–64 even states the identical false
blank-screen rationale. Thus f96dd436 removing an old-tmux-only exception does NOT introduce host contamination, change
that host promise, or newly remove a host acceptance. Earlier attribution to the assigned SPEC.md deletion is
superseded; original conditional route/consequence remains preserved. Distinct in-range documentation surface:
1ed482dde220452e424973faf38a5e1318986333 adds the authoritative implementation-spec paragraph (SPEC_impl.md at that
commit; at f96dd436:2657–2668): ‘A host runs one child for its whole life, so everything on its screen is this launch's
output; the screen is quoted only once the host's exit record has been processed, which follows the child's drained
output, and a Restart's new host starts with a blank screen, so no quote reaches back into the previous run.’ Its
replaced implementation-spec excerpt paragraphs concern tmux fences and make no such host blank-screen proof; the base
host overview says only that one child permits screen quoting without a fence. The added rationale copies an
already-false code comment rather than introducing a runtime change. It is nevertheless a source-refuted statement newly
placed in the implementation specification during the full pinned range. f96dd436 leaves that paragraph unchanged. This
is a distinct documentation correctness consequence (misleads future readers about launch-output provenance), not
evidence that the production behavior regressed in range. Current attribution and scope: no newly introduced defect
established in this assigned SPEC.md patch from f2. For the parent, retain the conditional runtime route as pre-existing
provenance and reconcile the new false-proof paragraph with the group owning 1ed482’s SPEC_impl.md excerpt section; do
not silently broaden this group’s source assignment. Current suggested bucket remains other. Confidence: confirmed
source/history comparison; no runtime execution. proposed_drop: attribution-only proposal for treating the runtime route
as a new defect of this assigned SPEC.md change; NOT a proposed drop of the separately newly authored false-proof
paragraph. No parent drop accepted. possible_cover: neighbours g184/f1 and g161/c1 rely on the same visible-screen-only
inference but do not include the resize/history-promotion route; these are interface pointers, not coverage judgments.
No missing source premise or open lead remains for this attribution check. Turn-5 named-input synthesis: read the exact
supplied g100/c1 latest record completely, including its exclusive-base path proof. It corroborates the previous turn’s
attribution conclusion, not a new contract change: both actual host screen-quote behavior AND the SPEC.md host
attribution promise/false parenthetical predate exclusive base 6e423e3. Removing the old-tmux-only exception at f96dd436
does not change the host promise, so the neighbour phrase that g013 owns a later caveat/promise change must not be read
as introduction of host contamination or of a new host guarantee. g009/l1 in the updated neighbour snapshot now
independently records the distinct in-range authoritative SPEC_impl.md paragraph at 1ed482 and the same false proof;
that is the appropriate owning-record reconciliation for that documentation surface. This f2 retains its original
conditional diagnostic consequence, full source route and original/revised proposed-drop bases; no independent runtime
cause or new finding is claimed, no parent drop/lowering accepted, and no source premise reopened. Existing whole-base
comparisons in this record settle the named question without rerunning reproduction or widening scope.

Returned proposal (claims-006, event 16): **retain-verified**, suggested bucket **other**.

Retain the newly authored implementation-spec false proof as an in-range documentation defect. A blank initial visible
screen does not prove later visible rows belong to this launch: terminal growth can expose inherited history before a
shell-start failure is classified. The conditional runtime route and the SPEC.md promise predate the exclusive base, so
they are provenance, not a newly introduced runtime regression. The latest history points to g009/l1 as the possible
owning record, but that record is outside this batch and was not independently checked as an alias.

Original stronger consequence: The considered diagnostic consequence was always other: a failed Restart can durably
quote unrelated previous-run output as this failed launch's last output. The first candidate called that route refuted
by visible-screen-only reads and blank seeding; later events explicitly withdrew the refutation. The initial attribution
to f96dd436's SPEC.md caveat deletion was later withdrawn after exclusive-base comparison. The conditional route itself
remains supported, but predates the range; the distinct false implementation-spec proof was newly authored at 1ed482. No
established loss of user work or security route was ever claimed.

Unresolved premise or limitation: No missing source premise for the documentation contradiction. Runtime manifestation
remains conditional on retained history, a height increase after publication but before classification, and a login
shell exiting before consuming the launch spec without overwriting all promoted rows. No actual occurrence, frequency or
stronger security/user-work consequence was established. Parent should reconcile the supplied g009/l1 pointer before
deciding which record carries the documentation finding.

Proposed basis: Record or reconcile one other-bucket in-range documentation finding for SPEC_impl.md's invalid
blank-screen provenance argument, retaining the pre-existing conditional route as its concrete counterexample. Do not
record a new runtime regression against f96dd436; do not reinstate the superseded blank-screen refutation. Underlying
key permits reconciliation with the other batch owning g009/l1 without claiming an unchecked alias.

Existing parent disposition (event 16): verified. Parent read all primary revisions and focused claims-006 full checks,
corroborated by claims-004 g009/l1. The inherited-history resize/excerpt source route and SPEC.md promise already exist
at the exclusive base, but 1ed482 newly authors the false SPEC_impl blank-screen provenance argument. Retain that
in-range authoritative documentation defect with the pre-existing conditional route as counterexample. No new runtime
regression or security/work-loss consequence claimed. Current Planned empty, no unlanded plan, BUGS/queue/active-ledger
exact cover; diagnostic filter does not clearly cover a newly authored general false proof, so ambiguity requires
retention. No original stronger harm dropped.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g013 record](records/review-through-00de0b6c/groups/g013.json). Record ID: `f2`.

### g100/c1

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: after Restart, increasing terminal height while the
login shell is still starting can make a later startup error quote a previous run's message as that failed launch's last
output. This is a source-confirmed conditional interaction, not observed execution. Details: the seed intentionally
carries old primary history above a blank visible screen (terminal-emulator snapshot/v1/mod.rs:242-268). Vendor
grid/resize.rs:25-28 invokes grow_lines for a height increase; :46-71 promotes min(history_size, lines_added) history
rows into the visible grid. term/mod.rs:756-758 enables that primary-grid resize. host_launch.rs:508-518 supplies
initial_snapshot to the new host, :579-625 waits for host readiness/adoption, then :639-645 tracks the still-pending
launch spec and returns. host_restart.rs:294-305 confirms the socket; :363-388 publishes it before returning, without
waiting for the login shell to consume its launch spec. Thus a viewer may resize this new terminal during ordinary shell
initialization; handlers.rs:2081 forwards owned viewer resizes to the resolved HostTerminal, whose resize request/record
is applied to the emulator (terminal.rs:3872-3885,4063-4070). If the shell subsequently exits without consuming the spec
or overwriting all promoted rows, status.rs:630-652 recognizes the unconsumed-spec/no-sentinel failure and calls the
excerpt helper. screen.rs:50-96 includes those visible rows, and launch_artifacts.rs:97-105 selects the last five
nonblank rows, so blank rows below the promoted old output do not protect attribution. No row provenance check
distinguishes inherited history from this launch's output. The helper still requires processed exit evidence; that
proves output ordering, not provenance of cells imported by the seed. Original c1 consequence and earlier proposed-drop
basis preserved: the concern was quoting a running/previous agent's screen, and earlier revisions proposed a drop based
on processed final-exit evidence, one-child host lifetime and blank Restart seed. Those checks correctly refute
premature process-probe quoting, but do not refute this history-promotion route; the general source-refuted conclusion
is withdrawn. Contract: SPEC.md:2890-2893 requires omitting an excerpt that cannot safely be attributed to the launch.
SPEC_impl.md:2759-2763 and the newly rewritten helper docstring at :44-56 claim the blank new screen prevents quotes
from previous runs; resize makes that invariant false. Remedy direction: track inherited-history provenance through
promotion, or conservatively omit the excerpt once inherited content may have entered the screen. Whole-range
attribution checked: the excluded base 6e423e30068b62c6cfa276c6d720ee35bb70387a already has the same host screen quote
and blank-seed rationale in launch_artifacts.rs:52-101; grid/resize.rs has no diff from that base to this commit, and
only comments differ in the history encoder. Therefore do not claim this helper move newly introduced the runtime route.
The assigned rewrite retains/reasserts the false attribution rationale while moving the helper, and g013/f2
independently owns the later in-range SPEC caveat-removal/promise change at f96dd4363d97dc7e07dea5b8947c96be6bf912e8.
Parent should reconcile that shared cause and attribution rather than count this as a separate newly introduced bug.
Suggested bucket: other (misleading durable launch diagnostic). Could-be-highest: no established security or
loss-of-work route. Confidence: confirmed conditional source/control-flow route; requires retained old history, a height
increase after publication and before classification, and shell startup failure leaving a promoted row visible. No
special seam, second failure or runtime reproduction was used. proposed_drop: false; earlier proposed source refutation
is withdrawn and no parent drop accepted. possible_cover: g013/f2 is the same source contamination route; no forbidden
coverage sources searched. Distinct l1 bounded excerpt-copying finding remains unchanged. No unresolved readily
checkable premise remains for this conditional route.

Returned proposal (claims-065, event 11): **needs-parent**, suggested bucket **other**.

The resize route defeats the blanket provenance claim. Its runtime mechanism and helper rationale predate the range,
while an in-range SPEC change removes an old-terminal exception and retains the unconditional host guarantee. Parent
must reconcile the separate owner and range attribution.

Original stronger consequence: Failed launch may quote previous-run messages. Earlier source refutation based on
processed exit, one-child lifetime and blank initial screen is withdrawn: those establish lifetime/timing, not
provenance after inherited history enters the screen. No security or loss-of-work consequence was alleged.

Unresolved premise or limitation: Parent must match g013/f2 and choose a single recording that identifies the in-range
documentation/contract defect without claiming a newly introduced runtime route. The exact adverse execution was not
observed.

Proposed basis: Keep the source-confirmed conditional provenance contradiction available for parent reconciliation.
Distinguish old runtime mechanism, changed/reasserted documentation and possible diagnostic-filter coverage.

Existing parent disposition (event 11): duplicate. Parent read full claims065 and compared canonical
g013/f2event16/prior parent judgment. The runtime resize-promotes-inherited-history path and code rationale already
exist at exclusive base; retain the newly authored1ed482 SPEC_impl blanket blank-screen provenance proof as the in-range
authoritative documentation defect. f96dd436's removed old-terminal exception is not acceptance of the host mechanism
and does not introduce that runtime failure. Same false provenance guarantee/counterexample as g013/f2 and g009/l1, with
all owner IDs retained, no extra item. Diagnostic-filter ambiguity does not cover the new false proof. No highest
consequence or observed execution. Source resize/launch-unwind/diagnostic-selection evidence is preserved; correct
processed-exit ordering alone is not provenance.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g100 record](records/review-through-00de0b6c/groups/g100.json). Record ID: `c1`.

### g110/c2

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. TLDR: Considered whether removing the per-launch excerpt
fence can attach an earlier run's output to a new launch failure. Source: the assigned patch removes
RunCells.launch_excerpt and its fresh initialization from relaunched_entry. The replacement append_launch_excerpt
(service/launch_artifacts.rs:42–76) reads only the held host's visible screen after HostEvidence::Owned { dead: true },
not retained history; status.rs:637–652 invokes it only for the inferred never-started shape. Host terminals run one
child and Restart seeds only scrollback above a blank screen, as specified in SPEC_impl.md, Launch failure
classification: "A host runs one child for its whole life, so everything on its screen is this launch's output" and "a
Restart's new host starts with a blank screen, so no quote reaches back into the previous run." Thus the old tmux
pane-reuse provenance concern has no new-launch route in this type narrowing. Suggested bucket: other (misleading
failure detail if a route existed); could-be-highest: no established security or loss-of-work route. Confidence:
confirmed source/contract analysis, no execution. proposed_drop: source refutation and explicit quoted host contract.
possible_cover: none. Caveat: this does not audit the entire restart rollback implementation; host-restart
implementation belongs to adjacent groups. Synthesis, turn 2: source recheck follows the seed end to end:
core/host_restart.rs:450–456 asks old.history_seed, hosts/terminal.rs:3733–3761 encodes through
emulator.history_seed_until, snapshot/mod.rs:541–569 selects the primary history, v1/mod.rs:246–267 calls paint_history,
and v1/paint.rs:417–424 truncates rows to history and appends freshly blank screen rows. host_restart.rs:216–255
supplies that blob to the new host spawn; absent seed defaults to None. append_launch_excerpt was reread at
launch_artifacts.rs:59–76 and still gates the screen-tail read on Owned/dead evidence. This establishes the
blank-new-screen/final-screen basis in source, rather than relying only on comments or the retired tmux fence. Parent
reports g100/g106 agree; that corroboration is not substituted for these source checks. Original consequence and narrow
scope remain unchanged; no drop verdict accepted. Discovery pass 2 interface update: exact neighbour g100/c1 now
establishes a distinct possible history-promotion route: growing the new terminal height can pull inherited old history
into its visible screen before a login-shell failure. The prior blank-seed/final-exit checks remain valid for the
boundary they checked, but do not alone refute post-seed resize contamination. Preserve all original wording and
proposed-drop basis above; the general dismissal is provisional, no parent drop accepted. Received references: vendor
grid/resize.rs:46–71 promotes history, term/mod.rs:756–758 enables primary-grid growth; shell startup may remain
unsettled after new host publication, and append_launch_excerpt reads the final visible rows. Need independently inspect
these useful accessible source premises and scoped attribution before finalizing. Suggested bucket remains other;
could-be-highest: no established security/work-loss route. Confidence: unresolved source lead received from g100, no
execution; not yet independently validated in this pass. possible_cover: g100/c1 and its named g013/f2 share the
contamination cause; no prohibited sources searched. Discovery pass 2 resolution: independently checked
vendor/alacritty_terminal/src/grid/resize.rs:25–28,46–71: a height increase grows the visible buffer, pulls
min(history_size, lines_added) rows from history and decreases the scroll limit. term/mod.rs:756–758 applies this to
primary grids. host_restart.rs:294–305,363–388 confirms/publishes the new host without an agent-shim-consumed fence.
handlers.rs:2071–2083 forwards an owned viewer resize; hosts/terminal.rs:3872–3884 applies the resize to the emulator.
launch_artifacts.rs:63–76 checks only final-screen evidence and reads screen_tail; :97–102 filters blank lines then
chooses the last nonblank rows. Therefore a retained old-history row promoted during startup can remain visible and be
quoted after the login shell fails without consuming its spec. A processed final exit settles output order, not
provenance. Confidence: confirmed conditional source route, no execution. Exact premises: retained history, height
increase after replacement publication, login-shell failure before consuming the launch spec, promoted old text not
overwritten. Original misleading-error consequence remains other, could-be-highest no. proposed_drop: false for this
provenance interaction; the prior broad refutation is withdrawn, preserving its original exact bases above. Attribution
refinement: at exclusive range base 6e423e30068b62c6cfa276c6d720ee35bb70387a, launch_artifacts.rs:52–95 already uses the
identical host final-screen/blank-seed rationale and reads host.screen_tail without the RunCells fence; that fence was
used only by the tmux arm at :98–106. Thus the assigned core field removal does not newly introduce the host route. This
record is reconciliation with received g100/c1 (and its supplied g013/f2 same-cause attribution), not a new independent
assigned-group discovery or a request to restore the retired tmux fence. The narrow deletion-attribution refutation
remains valid; it cannot support the former universal claim that a host quote never reaches old history. No unverified
source premise or open lead remains; parent owns consolidation, no drop/lowering accepted.

Returned proposal (claims-071, event 12): **out-of-range**, suggested bucket **other**.

The conditional resize contamination mechanism is real in source, so the broad blank-screen refutation was withdrawn
correctly. However, the host excerpt arm already had the same behavior and rationale at the exclusive range base;
removing the tmux-only fence does not introduce it.

Original stronger consequence: An earlier run's output can be attached to a new launch's durable failure detail after
inherited history is promoted by a resize. No security or user-work-loss consequence was claimed. The earlier universal
provenance dismissal is withdrawn; only the narrow deletion-attribution refutation survives.

Unresolved premise or limitation: Parent may hold a separate in-range introduction under the history-supplied aliases;
that evidence was not assigned. This result establishes only that the named fence deletion did not introduce the host
route and the relevant source existed at the range base.

Proposed basis: Treat this named deletion attribution as out of range, and reconcile the conditional mechanism with the
supplied same-cause identities. Do not use the withdrawn blank-screen rationale as a general source refutation.

Existing parent disposition (event 12): duplicate. Parent read complete claims071 and reconciles canonical g013/f2 and
g100/c1. This candidate correctly withdraws blank-screen universal runtime refutation and proves host inherited-history
promotion predates the exclusive base; fence deletion does not introduce it. The retained group concerns newly
authored1ed482 SPEC_impl false provenance proof, supported by that older runtime counterexample. Preserve this
candidate's precise old-runtime attribution and separate new authoritative wording; no extra queue item or claim that
runtime is correct. No highest consequence/observed execution. Parent's prior verified documentation finding remains;
this is corroboration of its limits.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g110 record](records/review-through-00de0b6c/groups/g110.json). Record ID: `c2`.

### g117/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (candidate, open):

Reviewed interface revision: f96dd4363d97dc7e07dea5b8947c96be6bf912e8; local assigned capture/comment revisions remain
1ed482dde220452e424973faf38a5e1318986333, f560058da19e02716e6f943883a9508b447aff4e and
08399d092d20815ced6619c26df2391944ae2b4a. TLDR: initial history-only seeding with a blank screen cannot refute an
old-run excerpt after the terminal grows. This is the already supplied g013/f2 interface finding, not an independent
discovery from the assigned capture patches. Local source check: vendor/alacritty_terminal/src/grid/resize.rs:25-28
calls grow_lines for increased height; :46-71 consumes history into the visible area. launch_artifacts.rs:44-59 claims
blank-screen seeding prevents prior-run quotes, while :68-74 reads the final visible screen and :96-102 selects nonblank
lines. handlers.rs:1881-1887 resizes on attach and :2042-2054 forwards owned resize without a launch-spec-consumption
prerequisite. These checks support the caller's history-promotion counterexample; no blanket initial-seed/final-screen
refutation is retained here. Original suspected consequence: misleading durable startup error quoting the previous run,
suggested bucket other; could-be-highest: no established security or work-loss route. Confidence: confirmed source
support for history promotion and excerpt selection; g013 supplies the full publication/login-shell failure path and
owns its finding. proposed_drop: no drop of g013/f2; no separate local report because this exact cause was supplied
before discovery, and the assigned service/capture.rs patches change first-submitted-input documentation/test fixtures
rather than excerpt provenance. This scope reason is not a claim that the production issue predates the full range or is
harmless. possible_cover: exact caller-supplied g013/f2. No original local excerpt-related withdrawal or seed-provenance
finding existed to retract; c3's unrelated old-terminal diagnostic-documentation finding and its later correction remain
intact. No execution/reproduction. No unresolved local check for this narrow interface reconciliation.

Returned proposal (claims-076, event 14): **needs-parent**, suggested bucket **other**.

The narrow interface evidence is verified: a blank initial screen does not guarantee excerpt provenance after growth.
The parent-supplied g013/f2 owns the complete publication path; its history was not supplied in this batch, so an exact
alias or whole-finding disposition cannot be certified here.

Original stronger consequence: A durable startup error can misleadingly quote the previous run. No established security
or work-loss consequence. No local blanket seed-provenance refutation is retained.

Unresolved premise or limitation: Parent must match g013/f2's full record, including restart seeding and a never-started
classification that actually publishes the promoted text. No runtime reproduction was performed.

Proposed basis: Carry verified history-promotion/excerpt-selection evidence into g013/f2; do not create a second local
finding merely for supplied interface synthesis.

Existing parent disposition (event 14): duplicate. Parent read full claims076 and reconciles g013/f2 canonical
source/contract proof and g100/c1. This owner contributes exact f96dd436 history-growth, owned-resize and nonblank
excerpt evidence defeating universal blank-seed provenance; full runtime route is older than range, while1ed482 newly
authors false SPEC_impl proof. Keep all references as corroboration of retained documentation defect, not an independent
duplicate or new runtime regression. No highest consequence/runtime observation; unclear diagnostic filter does not
cover new false proof.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g117 record](records/review-through-00de0b6c/groups/g117.json). Record ID: `c4`.

### g161/c4

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 12 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original TLDR preserved: Considered whether removing
resize-time excerpt invalidation lets an error for a failed Restart quote the previous run's output. Original suspected
consequence: a misleading failure explanation, bucket other; could-be-highest no established security or loss-of-work
route. Current TLDR: after Restart inherits history, increasing terminal height while the login shell is still starting
can expose old history on the new screen and make a subsequent startup error quote it as this launch's output.
Confidence: confirmed conditional source route, not execution. proposed_drop: false for the broad excerpt-attribution
claim; original source-refuted proposal is preserved narrowly because deleting the tmux codec/resize fence does not
introduce a reused-pane obligation into the retained host path. No drop accepted. Source checks this turn:
vendor/alacritty_terminal/src/grid/resize.rs:25–28 grows height through grow_lines, whose 57–71 pulls retained history
into the visible grid and shifts the cursor. host_launch.rs:508–518 includes initial_snapshot, 579–625 waits for the
host start/adoption only, and 639–645 tracks the still-pending launch spec and returns the host. The already-read
host_restart.rs:294–388 confirms and publishes that host. handlers.rs:2060–2083 resolves the selected runtime and
forwards an owned viewer resize without waiting for launch-spec consumption. Prior reads established
launch_artifacts.rs:59–76 quotes owned-dead final visible screen and 96–123 removes blank rows before retaining five; no
inherited-row provenance gate exists there. Thus processed exit evidence and a blank initial screen prove neither
single-launch cell provenance after growth nor safety of the broad attribution assertion. The surviving seed test
snapshot.rs:1263–1311 checks immediate restoration blankness/default modes and retained history, but does not grow the
seeded terminal before reading the error excerpt. It cannot refute this distinct interaction or serve as its regression
oracle. Range attribution check: the excluded base 6e423e30068b62c6cfa276c6d720ee35bb70387a already has the same host
visible-screen excerpt at launch_artifacts.rs:80–94 and the same blank-seed rationale at 52–64. The whole base-to-tip
diff is empty for vendor grid/resize.rs and changes only an unrelated re-export in snapshot/v1/mod.rs. Therefore do not
attribute introduction of the runtime route to the assigned tmux deletion. possible_cover: g100/c1 and g013/f2 describe
this same inherited-history resize route; g013 owns the later in-range specification caveat/promise change. This is
reconciliation of known interface evidence and the original c4, not new discovery or an additional cause. Exact
conditions: retained history, height increase after host publication before failed-launch classification, shell exit
leaving the launch spec unconsumed and some promoted rows unoverwritten. Source checks cannot establish occurrence or
frequency, and neither is claimed. Remedy direction if retained: preserve provenance across history promotion or
conservatively omit the excerpt after inherited history may have entered the screen. All readily checkable premises for
this conditional route are settled; no open lead remains. No runtime test, reproduction or live-install action.

Returned proposal (claims-102, event 12): **retain-verified**, suggested bucket **other**.

The conditional inherited-history route is supported by source. Runtime machinery predates the range, but the in-range
authoritative specification newly presents an unqualified no-previous-run excerpt rationale, and that promise remains
current. Do not restore the withdrawn broad refutation.

Original stronger consequence: A failed Restart's explanation may quote previous-run output. Other throughout; no
security or user-work-loss route claimed. The original broad resize exclusion was withdrawn; only the narrow absence of
a reused-tmux-pane obligation remains refuted.

Unresolved premise or limitation: None for the conditional source/contract contradiction. Actual occurrence and
frequency remain unobserved; the route needs inherited history, a height increase while the new host is published, a
login-shell exit with spec unconsumed, and promoted rows left unoverwritten.

Proposed basis: Retain the in-range authoritative documentation defect plus conditional runtime explanation, explicitly
noting runtime preexistence. Reconcile under the underlying key with the supplied g013/f2 and g100/c1 histories in their
owners' batches.

Existing parent disposition (event 12): duplicate. Parent read full102 source/coverage and earlier013f2 canonical. Same
inherited restart history promoted by resize into final visible excerpt; actualruntime/blank-seed code rationale
prebase, but1ed adds false authoritative no-previous-run assurance toSPEC_impl. Preserve supported conditional sequence,
no observed frequency, no need to invent newruntime introduction or tmux-pane reuse. Combine with013f2
and100c1/110c2/117c4 provenance, not another queue defect.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g161 record](records/review-through-00de0b6c/groups/g161.json). Record ID: `c4`.

### g165/c3

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 11 (finding, open):

Reviewed commit: 1ed482dde220452e424973faf38a5e1318986333. Original considered consequence preserved: removal of
reused-pane tests might let a failed new launch quote the preceding conversation or tmux's exit banner. TLDR: A normal
height increase after Restart can bring inherited scrollback onto the visible screen, so a later quiet startup failure
can quote a previous run's output as its explanation. This is known cross-group input g013/f2 and g100/c1, now validated
here, not a new discovery. Source route: host_restart.rs:237-256 supplies the old host history seed; the seed painter
snapshot/v1/paint.rs:404-436 initially paints history above blank screen rows. host_launch.rs:508-518 puts it in the new
ChildSpec; 579-625 awaits host startup/adoption only; 639-645 returns while tracking the pending launch spec.
host_restart.rs:294-305 and 363-388 confirms/publishes that host without waiting for the login shell to consume the
spec. An owned viewer's normal resize reaches terminal.resize at handlers.rs:2071-2082, then the runtime applies the
resize record at hosts/terminal.rs:3872-3884 and resizes its emulator at 4063-4070. vendor term/mod.rs:756-758 resizes
the primary grid; grid/resize.rs:25-28,46-71 pulls up to the added height from retained history into visible rows.
screen.rs:50-94 reads all visible rows, so the helper at launch_artifacts.rs:63-74 can read the imported rows after
Owned dead evidence; its reducer at 96-123 ignores blank rows and selects the last five nonblank rows. A shell that
exits before consuming the spec and leaves those rows intact reaches status.rs:624-652's never-started excerpt
classification. Final dead evidence orders current output but supplies no row-provenance fence. Source-only conditional
route; no execution or reproduction. Whole-range attribution: exclusive base 6e423e30068b62c6cfa276c6d720ee35bb70387a
already has this host screen-only quote and the same blank-seed rationale at launch_artifacts.rs:52-94. Diff from that
base to the reviewed deletion has no change in vendor grid/resize.rs and no change in paint_history (the paint.rs diff
only removes unrelated sgr_setting). Therefore this assigned test deletion does not newly introduce the runtime route.
It removes old tmux-specific conservative resize/fence tests while the helper rewrite carries an already-false blanket
provenance explanation; g013/f2 owns the later in-range contract caveat-removal at
f96dd4363d97dc7e07dea5b8947c96be6bf912e8. Parent should consolidate provenance rather than count this as a newly
introduced independent bug here. Earlier proposed-drop bases preserved but corrected: current-host identity, one child
per host, initial blank-screen seed and processed final exit are all true; they refute an obsolete observer quoting a
later host and the tmux exit-banner mechanism, not history promotion on resize. The retained seed test at
host_terminals.rs:3803-3841 asserts blankness only immediately after restoration; the failure observer test at 184-218
supplies no inherited history or height change. Their actual assertions therefore do not cover this interaction. Correct
production was never a substitute for those test-oracle checks. The prior blanket source-refuted conclusion is withdrawn
under this original ID. Sanitizer/500-byte result bounds remain separately preserved under c1; this finding does not
allege their loss. Contract: SPEC.md Terminal experience at the reviewed revision says Farhelm omits excerpts it cannot
safely attribute to that launch, and a new terminal never shows a previous run's screen; SPEC_impl.md Restarting a host
agent and launch_artifacts.rs:44-56 use the initial blank screen to justify provenance. That reasoning does not survive
growth of seeded history. Suggested bucket: other, misleading durable launch diagnostic. Could-be-highest: no
established security or user-work-loss route for this attribution concern. Confidence: confirmed by source/control flow,
conditional on retained history, a height increase after publication before classification, and startup failure leaving
an imported row visible. proposed_drop: false; earlier source refutation withdrawn, no parent drop accepted.
possible_cover: exact known cross-group records g013/f2 and g100/c1; no prohibited coverage search. All readily
accessible source premises for this narrow conditional route are settled; no open lead remains. Remedy direction:
preserve inherited-row provenance through resizing or omit excerpts once inherited history can enter the screen. No
request to restore retired tmux implementation or its banner-specific tests.

Returned proposal (claims-104, event 11): **needs-parent**, suggested bucket **other**.

The conditional same-session history-promotion route is source-established, and the prior blanket refutation is
withdrawn. Its runtime and blanket blank-seed rationale already exist at the exclusive base. Preserve it for
consolidation with the separately owned in-range contract claim rather than treating this test deletion as a newly
introduced runtime bug.

Original stronger consequence: A failed new launch could quote the preceding conversation or tmux exit banner. The
tmux-banner/reused-pane mechanisms are absent; inherited same-session history can still be quoted. Earlier broad source
refutations are withdrawn for that latter route, not silently treated as accepted drops. No cross-session disclosure or
user-work-loss route was established in this record.

Unresolved premise or limitation: The runtime route is conditional, not reproduced. Parent must resolve whole-range
scope with the supplied g013/f2 and g100/c1 owners: no new runtime introduction is established here, and the host
provenance assertion also predates the range. Whether in-range rewriting/caveat deletion itself warrants a documentation
finding is not settled by this consultation.

Proposed basis: Consolidate the established conditional mechanism and preserve range/filter uncertainty. Do not queue
this deletion as an independently introduced runtime defect or silently discard the related in-range contract question.

Existing parent disposition (event 11): duplicate. Parent resolves104 needs-parent using actual earlier013f2/new1edSPEC
attribution plus102same source chain. Same history-seed/height-growth/final-excerpt mechanism, currentSPEC_impl new
false attribution assurance supplies in-range documentation claim while runtime is prebase. Preserve prior blanket
refutation withdrawal, not a separate newly introduced product bug or filter on merely diagnostic outcome.
Source-confirmed conditional mechanism remains in canonical record with all inherited-history/failure prerequisites.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g165 record](records/review-through-00de0b6c/groups/g165.json). Record ID: `c3`.

# Stop-test migration removes the signal-to-legacy-client teardown oracle

Reviewed commit: 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db

## TLDR

Unverified collection: at this intermediate revision existing tmux sessions remain supported, but the three real
supervisor-stop tests no longer check that stopping the supervisor safely quiets both output-bearing tmux clients before
exit. The host-survival check exercises a different substrate, so an omission in the legacy signal-to-cleanup path can
pass these tests. Established defect is lost end-to-end regression coverage, not a current crash or lost user work.

## Details

Collected immediately at the maintainer's request from the pinned review. No further source investigation, coverage
search, claim check, independent dismissal check or exhaustive reconciliation was performed. This entry preserves
proposals rather than approving a dismissal or severity reduction. It has not been reassessed against product changes
after the pinned tip.

Provisional priority: **other**. Original and later consequence statements are preserved separately below; this bucket
does not approve any proposed lowering.

Contributing records: `g140/f5`.

### g140/f5

Confidence: Unverified collection; recording proposals and any disagreement remain unresolved.

Latest reviewer record, event 14 (finding, open):

Reviewed commit: 647f0227b7f5b6c5fe443b2d5f0c4a20de0d5513. TLDR: at this intermediate revision existing tmux sessions
remain supported, but the three real supervisor-stop tests no longer check that stopping the supervisor safely quiets
both output-bearing tmux clients before exit. The host-survival check exercises a different substrate, so an omission in
the legacy signal-to-cleanup path can pass these tests. Established defect is lost end-to-end regression coverage, not a
current crash or lost user work. Details: the assigned patch removes the --new-terminals tmux/--tmux-wrapper fixture,
no_output_recording_tmux and the old supervisor_stop.rs:262-283 assertion that at least two distinct clients
successfully acknowledge no-output after the STOP sentinel. The new :136-156 creates and attaches a host terminal, and
:216-233 asserts Host kind and live agent echo after process exit. The remaining log/status checks (:182-202) establish
entering the shared stop handler and no budget warning but cannot show the legacy forwarder and session sink crossed
their output-disabling boundary. The intermediate contract still requires legacy service (SPEC_impl.md:2954-2988) and
acknowledged no-output before closing output clients (:3134-3167). Actual surviving tests were inspected rather than
inferred from source correctness: tmux/stream.rs:2724-2833 gates a directly invoked OutputStream shutdown before/after
its acknowledgement and confirms client liveness then reap; this is valuable coverage of one driver method, but neither
sends a process signal nor drives the supervisor's two-client roster. teardown.rs:4912-4995 tests stop waiting for fake
sink leases/reapers, and :5005-5016 tests the attachment-lock budget; the fake sink at :4881-4898 has no tmux client or
acknowledgement. handlers.rs:7863-7904 proves a racing attach's sink settles after refusal, but asserts no real process
signal or distinct output-client acknowledgements. A pinned search for DisableOutputGate and disable_output_gate finds
gate use only in the direct stream tests. These checks do not preserve the deleted whole-path oracle.
core.rs:17353-17390 and teardown.rs:1571-1608 show the intended implementation still joins the orderly legacy cleanup,
which is correct source but not replacement test evidence. Suggested fix at the reviewed revision: keep a focused
signal-driven legacy fixture using a test-owned preexisting tmux session, alongside the host-survival cases, or provide
equivalent integration assertions connecting real stop triggers to both legacy output clients. Suggested bucket: other
(maintainer regression coverage); confidence: confirmed source/test-contract comparison, no execution. proposed_drop:
false. possible_cover: none known; no excluded metadata or queue searched. Could-be-highest: no established
security/user-work-loss route from test deletion; no current tmux abort claimed. Whole-range caveat: the following
in-range commit dbfc69751215dbf15e1d31d5bed73c3754c4ebc4 retires old terminals and
1ed482dde220452e424973faf38a5e1318986333 removes the driver. Thus this finding is specifically the supported-legacy
intermediate revision, not a demand to retain legacy tests after retirement or a claim that final-tip host cleanup lacks
tests. The later supervisor_stop.rs diff through 00de0b6c9cf206f1a7c9c5ad1c76b5b10eaf85db changes only the separate
startup-tether fixture; it does not restore this oracle before legacy removal. Original f5 lead is closed with these
useful checks; no missing source premise remains for the bounded coverage-loss conclusion.

Returned proposal (claims-089, event 14): **retain-verified**, suggested bucket **other**.

Host-survival assertions replace an end-to-end legacy shutdown assertion while existing legacy terminals remain
supported at the reviewed intermediate revision. Direct driver shutdown tests retain a useful narrower oracle. Later
legacy retirement bounds the finding's lifetime.

Original stronger consequence: Original lead was potential loss of orderly legacy output acknowledgements. Final
consequence remains lost regression coverage; no current crash, user-work loss, final-tip host cleanup gap, or need to
retain legacy tests after retirement is claimed.

Unresolved premise or limitation: No missing premise for bounded intermediate oracle loss. Parent must preserve that
later retirement makes a present-day legacy-fixture repair obsolete.

Proposed basis: Retain the historical source-established test gap separately from replay-marker loss; scope any queue
wording to the supported intermediate revision.

Existing parent disposition (event 14): rejected. Parent read full claims089.647f0227 converts stop-survival test
tohostliveecho and drops realsupervisorSIGSTOP/atleasttwoacknowledgedno-output-client roster assertion while existing
legacyservice stillrequires thatdiscipline. Direct drivergatedack and fakelease/reaper tests retain narroweroracles, not
the samewholepath. Preserve intermediate losttestprotection, no currentabort orworkloss allegation.
dbfc6975retireslegacy and1ed482 removesdriver bytip; no presentobsoletelegacyfixturefix queued. Distinct
fromdeepreplaytail loss and currenthostcleanup behavior.

Complete original wording, every revision/withdrawal, cited checks and exact cover quotations:
[g140 record](records/review-through-00de0b6c/groups/g140.json). Record ID: `f5`.

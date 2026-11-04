# Recovered agents leave shutdown checking an ended launch handle

Reviewed commit: 6bd019f5359ed2ec4c69b13ab75b4840626f842f

## TLDR

After an agent terminal recovers, a later Mac supervisor replacement can unnecessarily wait three seconds and warn that
the session never secured its installed version. The session may already have secured it; shutdown is checking the ended
terminal handle instead of the recovered one. This is a source-confirmed conditional route, not a measured runtime delay
or actual version-lock loss.

## Details

Suggested bucket: **other**, for a bounded update delay and misleading warning. Confidence: **confirmed source route**.
Proposed drop: none.

`crates/farhelm-supervisor/src/service/core/host_launch.rs:632` tracks the newly created runtime for the version-hold
wait. In the versioned app layout, `service/core.rs:8018–8030` stores its `Terminal` clone in `pending_holds`. Entries
whose hold mark has appeared are pruned only when another launch calls this registration function. The final launch can
therefore remain tracked long after its valid mark exists.

Midlife recovery replaces the session entry's runtime at `service/core/host_handover.rs:1578–1588` but does not update
that captured handle. The ended original runtime permanently reports unavailable
(`hosts/terminal.rs:803–825,2448–2449`). `hold_settled` at `core.rs:8115–8132` gets that error before comparing the
saved mark with the live child. The wait at `8047–8101` stays unsettled until its deadline and emits the
unprotected-launch warning at `8090–8096`. `stop_in_order` at `18734` uses the three-second budget defined at
`18751–18757`.

A sufficient sequence is to create a session in the versioned app layout, let its hold mark appear, recover its agent
terminal after a contact failure, start no later session, and eventually replace the supervisor normally. No continuing
failure at shutdown is needed. Another launch can prune the old entry; a supervisor restart clears the in-memory list.
Supervisors outside the versioned layout do not register these holds.

SPEC_impl.md, "Side-by-side versions inside Farhelm.app", describes a bounded wait for genuinely pending version
protection, comparing the mark with the current terminal child's PID. It does not accept treating a recovered session's
ended handle as current. Bind the check to the launch's current runtime, or preserve validated child/hold provenance
across recovery. This does not challenge the fallback for a genuinely unreadable current host.

Source checks covered registration, recovery replacement, process probing, the wait and its stop caller. The captured
handle logic remains through `c5ae8548439cc5bfa33c11ef7ce8da6129d8072a`. No exact acceptance, planned repair, known bug,
queue item or ledger decision was found. The existing shutdown-wait test finding concerns an unrelated missing test
witness. FILTER.md's rare diagnostic rule does not clearly cover the additional stop delay, so the whole finding is
retained. No test, update or live-install operation ran.

Provenance: trailing-review run `ffa3b94e-5eb1-4569-8eb5-1a52b62110d1`, `g048/g048-d11`, event 20.

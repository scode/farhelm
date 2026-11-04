# A registry test's basename lookup can select a foreign terminal host

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

A registry test can identify a terminal host by its socket's basename and then kill it, even if that host belongs to a
different directory and holds unrelated work. The conditional wrong-selection route is established; an actual collision
was not observed.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; foreign matching host required

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm/tests/terminal_host_registry.rs:62-66,131-136.

Distinguish selecting the wrong initial process from the separately accepted adjacent PID-reuse window. The production
full-path signal guard does not protect this test's bare selected PID. The naming-convention/tooling acceptance is not
an unambiguous waiver of foreign work loss; retain the independent ambiguity.

host_process at :62-66 calls socket_process; :131-136 passes its result directly to SIGKILL. At the introducing commit
hosts/process.rs:173-185,188-234 explicitly matches only the socket's file name across directories and returns a
matching same-user process; its module contract at :26-34 says the loose match is safe for conservative unavailable
classification, whereas a signal needs exact spelling. Forty-bit random names reduce collision frequency but do not
distinguish two already running hosts with the same basename in separate directories.

A test running beside another checkout or a real Farhelm installation can kill that other directory's terminal host if
their randomly drawn socket names collide. The other terminal's program receives its hangup, so this can stop user-owned
work. Exact source at tip: terminal_host_registry.rs:62-66 wraps socket_process and :131-136 signals its result
directly. hosts/process.rs:173-185 matches only the final pathname component; :207-234 returns the first matching
same-user live process. procs.rs:92-100,129-131 makes that scan a HashMap, with no preference for the caller's
directory. hosts/names.rs:32-45 draws forty random bits. hosts/spawn.rs:334-348,612-625 checks only each directory's
full path, so two ordinary hosts in different directories can coexist with the same basename. If the other process is
encountered first, it is signaled before any peer assertion. terminal-host/SPEC.md:149-166 establishes that SIGKILL
closes the PTY master and gives its child a hangup. No crash, PID reuse, malicious replacement or concurrent starts in
one namespace are required.

Possible coverage and caveats: that naming-convention paragraph; SPEC_impl.md:1788-1803 instead explains that
basename-only matches are conservative candidates and signals require the exact socket spelling. The test comments
:11-17 assume random naming prevents cross-run matches, but randomness only makes them rare. Searches of SPEC.md,
SPEC_impl.md, TODO.md Planned (empty), plans/queue, BUGS.md, queue INDEX/matching records and TRIAGE_OUTCOMES.md found
no separate exact collision/registry-test acceptance, plan, bug or queued item. Filters for rare safe failures exclude
wrong-target actions and user-work loss. Fix direction: obtain the peer identity from a connection to this test's exact
socket or use exact-path verified signaling.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g116/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is c0a0c9fb126f0a6d05d72a245f19cd6288b11cbc
(introducing assigned patch), persists unchanged at ca9b0a19856dc136f63ad2d14f1076027edce1eb.. This was a source-only
review; runtime occurrence and frequency were not measured.

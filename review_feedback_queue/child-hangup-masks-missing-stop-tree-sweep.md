# Stop's descendant test does not distinguish a tree sweep from session-leader hangup

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

The Stop regression can credit the death of an unmarked descendant without distinguishing whether Stop swept that
descendant or merely killed its parent shell.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by static
source and documented Linux behavior; no executed mutation or measured false pass. Synthesis resolves the earlier
missing shell/process-group premise. At the tip, host_terminals.rs:82-87 invokes /bin/sh -c with no interactive or
monitor flag; :53-57 launches ordinary sleep 1000 in the background and waits. Read-only package metadata identifies the
available /bin/sh as /usr/bin/dash, package 0.5.12-6ubuntu5. Retrieved the matching official Ubuntu source descriptor
and both source archives from https://archive.ubuntu.com/ubuntu/pool/main/d/dash/. Verified archive SHA-256 values
against the descriptor: upstream 6a474ac46e8b0b32916c4c60df694c82058d3297d8b385b74508030ca4a8f28a, Debian changes
84671be9725b738725575d44069f9e7a41344ae6a8027d8ba2d6a172b5e406d6. Dash 0.5.12 src/options.c:119-188 leaves iflag/mflag
off for this -c invocation; src/jobs.c:188-241,751-784,849-944 creates separate process groups only under job control
and otherwise ignores only SIGINT/SIGQUIT for a background child, not SIGHUP. The Ubuntu patch series has no
jobs.c/trap.c patch; inspected all its options.c hunks and none changes these conditions. At the reviewed Farhelm tip,
crates/terminal-host/src/spawn.rs:531-580 establishes a new session, controlling PTY and default signal dispositions
before executing the shell. Linux v6.8 drivers/tty/tty_jobctrl.c:98-126,365-411 sets the controlling terminal's
foreground group to the acquiring shell's group; :265-284 sends SIGHUP to that group when the leader exits. Source:
https://github.com/torvalds/linux/blob/v6.8/drivers/tty/tty_jobctrl.c. The Linux man-pages project's setsid(2) NOTES
independently documents that termination rule: https://man7.org/linux/man-pages/man2/setsid.2.html. GNU coreutils v9.4
src/sleep.c:94-144 has no process-group or SIGHUP-disposition change before sleeping
(https://github.com/coreutils/coreutils/blob/v9.4/src/sleep.c; installed base version 9.4). Therefore this ordinary
Linux fixture's shell and sleeper share the terminal foreground group, and a Stop that signals only the shell can still
satisfy both disappearance checks at :301-303 through the kernel's hangup. Host survival at :310-312 does not
distinguish that mechanism: the PTY master need not close. This establishes a weak test oracle, not a defect in shipped
Stop. Use the existing hangup-immune fixture at host_terminals.rs:422-480 and check parentage/SigIgn before Stop, while
preserving final-screen and host-survival assertions. Coverage: retained prior full coverage checks; SPEC.md:1356-1358
at the tip promises whole-tree Stop, SPEC_impl.md:1973-2003 specifies the child-pinned root, neither accepts this test
gap; TODO Planned is empty; tab-output-hangup plan:71-97 concerns other tab termination; no exact BUGS, queue, ledger or
filter coverage was found.

Source locations: crates/farhelm/tests/e2e/host_terminals.rs:53-57,293-303 at ca9b0a19856dc136f63ad2d14f1076027edce1eb.

tree_script creates a plain background sleep under noninteractive /bin/sh, with no HUP trap; Stop at
host_terminals.rs:257-267 waits for the shell and sleeper to disappear and checks the host remains alive. Inspected
supporting source at e354c3287db51075ae463dc61df5e257afabb6db: crates/terminal-host/src/spawn.rs:531-580 makes the shell
its own session leader with a controlling PTY and resets signal dispositions. Killing that leader can send the
foreground group SIGHUP, so if its background sleep shares that group, a sweep that only killed the shell could still
satisfy both disappearance assertions. Delete's fixture at host_terminals.rs:371-429 deliberately makes its descendant
HUP-immune and checks parentage and SigIgn before deletion; Stop still uses tree_script.

Possible coverage and caveats: none established.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g142/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb.
Introduced in assigned patch 7962307589c2a87859fdca3a0eea788fb2389f17 (host_terminals.rs:40-48,257-267).. This was a
source-only review; runtime occurrence and frequency were not measured.

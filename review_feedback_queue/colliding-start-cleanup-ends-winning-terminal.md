# Cleanup of a colliding unfinished start can end the winning terminal

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

An interrupted terminal start that collides with another terminal's socket name can close that other terminal and hang
up its work. The ordinary foreground Ctrl-C route remains outside any clearly matching accepted exception.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; random collision and timing required, no observed occurrence

Source locations: crates/farhelm-supervisor/src/hosts/spawn.rs:385-412@1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:443-479,568-587.

Completed explicit refusal bypasses cleanup. Two ordinary creates are serialized; independent session tabs and
create-versus-tab overlap supply the relevant callers. Foreground Ctrl-C or orderly supervisor SIGINT can interrupt the
first stage while cleanup runs. Exclude only a genuinely independent exceptional second-fault subset under
SPEC1544–1556. Deliberate private-directory interference is a different trigger. Unreadable-winner signal fallback adds
its own condition; the raw untagged Shutdown proof does not establish that extra variant.

At introducing commit 1512cdb6e7db601f8b07cfe596bddf101b1a3e45, hosts/spawn.rs:429–443 selects a random socket name by
checking absence without reserving it; :312–319 reaps an unfinished first stage and :385–412 sends an untagged takeover
and Shutdown to whatever answers. At pinned tip, free_name:612–625 and finish:443–479 retain that shape; :568–603 sends
the cleanup Shutdown. A completed nonzero refusal at :445–454 returns safely. The harmful route therefore requires a
losing starter interrupted before its refusal is observed. Two ordinary creates are serialized; different sessions' tab
opens, or a create and a tab open, can overlap through shared host_scan reads (core.rs:15178–15183,15320–15344;
core/host_tabs.rs:619–652). Foreground supervisor launch at main.rs:1747–1834,2112–2120 does not detach its first stage,
which inherits the process group (hosts/spawn.rs:350–360). The synchronous terminal-host first stage has no SIGINT
handler; only its second stage detaches (terminal-host/src/spawn.rs:32–123,137–155). An ordinary foreground Ctrl-C can
therefore end the unfinished losing stage while the supervisor catches SIGINT and runs orderly cleanup
(core.rs:18652–18685,18704–18726). If its forty-bit random name collided with an independently detached winner, cleanup
closes that winner's PTY and can hang up its work. No deliberate private-directory substitution is needed. Reserve
socket ownership or validate the peer's launch tag before cleanup. Actual collision and timing were not observed.

Possible coverage and caveats: newly named SPEC.md:1544-1556 (plausible exceptional-failure subset; application to
ordinary-stop timing is ambiguous), plus existing SPEC_impl.md:1763-1774 / terminal-host/SPEC.md:86-108 whatever-answers
mechanism. Those passages do not expressly name a differently tagged terminal being lost after an ordinary stop.
Source-supported scope and possible coverage must travel together; no unilateral exclusion or inference of maintainer
intent. Earlier coverage-source searches and other fields remain as recorded.

Additional reviewed locations: g006/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:334-348,443-479,568-587,612-625;
g007/f2: 1512cdb6e7db601f8b07cfe596bddf101b1a3e45:SPEC_impl.md:1688-1700;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:443-479,568-587,612-625; g008/f2:
SPEC_impl.md:1979-1993 at e354c3287db51075ae463dc61df5e257afabb6db;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:hosts/spawn.rs:443-482,568-603; g018/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:350-360,443-479,568-587; g020/f2:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:350-360,443-479,568-587; g091/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:443-483,568-587;
service/core.rs:11767-11787; g111/f1:
cf5188fd16a5cd4dec1e57fc257f0f08037f636d:crates/farhelm-supervisor/src/hosts/stop.rs:486-492;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:479,568-587; g144/f1:
cf5188fd16a5cd4dec1e57fc257f0f08037f636d:crates/farhelm-supervisor/src/service/core/host_launch.rs:498-545;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:host_launch.rs:527-574,746-780; hosts/spawn.rs:443-479,568-587; g145/f1:
e354c3287db51075ae463dc61df5e257afabb6db:crates/farhelm-supervisor/src/service/core/host_launch.rs:746-774;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:752-780; g149/f2:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/session_hosts.rs:392-430,761-766;
g179/f1: 7987d070e91684f4c52df7695136cb87e66bd94f:crates/farhelm-supervisor/src/service/core/host_tabs.rs:605-638;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/hosts/spawn.rs:443-479,568-587.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g019/f1`, `g006/f1`,
`g007/f2`, `g008/f2`, `g018/f1`, `g020/f2`, `g091/f1`, `g111/f1`, `g144/f1`, `g145/f1`, `g149/f2`, `g179/f1`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1512cdb6e7db601f8b07cfe596bddf101b1a3e45;
persistence and source extension checked at ca9b0a19856dc136f63ad2d14f1076027edce1eb. Existing conditional wrong-target
Shutdown remains the same underlying defect as g018/f1, not a new discovery ID.. This was a source-only review; runtime
occurrence and frequency were not measured.

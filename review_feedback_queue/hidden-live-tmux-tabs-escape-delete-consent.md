# Hidden live tmux tabs escape Delete's warning

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

After the agent moves to a terminal host and its host is lost, independent old terminal tabs can disappear from the
live-work count. Delete can then end those tabs without the required warning, although they may still hold work.

## Details

Suggested bucket: **highest**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source route; documented hidden-setting support scope remains ambiguous

Source locations: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/status.rs:531-542;
crates/farhelm-supervisor/src/service/core.rs:14720-14735.

The current binary can create requested agents and independent interactive login shells under its hidden tmux flag, then
migrate while retaining tabs. a023/a025/a041/a042/a071/a072 and renewed a045/f2 personally checked the complete
producer. Harness intent does not prove disposable work. Preserve exact claim-boundary migration-crash and proven
older-release-plus-additional-rare-fault exclusions, not a blanket old-release premise. Immediate Launching guard is
valid only until the ordinary Stop bridge; terminal=None then permits zero-count Delete. Near term TODO is not Planned.

At pinned tip, service/status.rs:531–542 discovers old tmux tabs through the current agent terminal. Once that terminal
is absent, the tab listing and Delete's live-work count omit surviving old tabs, while destructive cleanup still ends
their tmux session (service/core.rs:14720–14735). The current binary itself supplies a producer: core.rs:6164–6212
carries the hidden tmux switch into runtime construction, :11634–11678 launches the requested agent, and
:15320–15355,15400–15429,15504–15540 opens real independent shell tabs. launch.rs:740–778,1324–1348 and CLI
main.rs:1889–1896 execute the user's interactive login shell. Migration core.rs:13653–13702 retains these tabs while
replacing the agent with a terminal host. Losing that host and orderly reload can leave no agent terminal while the old
tabs remain live. Immediate Launching protection is narrow: core.rs:5484–5489,14655–14664 guards that state, but
ordinary Stop can leave the terminal-less session eligible for the zero-count Delete. Listing old tabs by durable
session identity rather than only the current terminal would preserve their visibility. Current hidden-flag support
scope is ambiguous, but its documented harness purpose does not prove that real user shells hold disposable work. Remote
connected uninstall is an additional destructive consumer (provisioning/service.rs:2692–2698), retained through g123/f3.

Possible coverage and caveats: the same-binary completed-migration then agent-host-loss/orderly-reload route is not
shown to meet either exclusion. Its harness-only intended scope is explicit, so whether its possible user-owned work
warrants highest or only tooling other is a scope decision, not missing source behavior. TODO Near term is not Planned;
the original Stop ledger consent rationale excludes the independent tabs that Stop preserves. Reverse flag switching
does not directly create tmux tabs for a host-born agent (core.rs:15249-15271 refuses it).

Additional reviewed locations: g047/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736; g048/f1:
crates/farhelm-supervisor/src/service/status.rs:531-542; service/core.rs:14720-14736; service/teardown.rs:754-803;
g049/f1: crates/farhelm-supervisor/src/service/status.rs:531-542;
crates/farhelm-supervisor/src/service/core.rs:14720-14735; g050/f1:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736; TODO.md:181-213;
g090/f1: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736,14778-14792;
g092/f1: ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736; g096/f2:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736; g098/f2:
fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm-supervisor/src/store.rs:4335-4365; g123/f3:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-helm/src/provisioning/service.rs:2692-2698; g149/f3:
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736; g184/f4:
fba73468a9cb09c217a39da2be7f2c7cea064cb8:crates/farhelm-supervisor/src/service/core/host_restart.rs:153-164;
ca9b0a19856dc136f63ad2d14f1076027edce1eb:crates/farhelm-supervisor/src/service/core.rs:14720-14736.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g046/f1`, `g047/f1`,
`g048/f1`, `g049/f1`, `g050/f1`, `g090/f1`, `g092/f1`, `g096/f2`, `g098/f2`, `g123/f3`, `g149/f3`, `g184/f4`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is ca9b0a19856dc136f63ad2d14f1076027edce1eb;
assigned seam 7962307589c2a87859fdca3a0eea788fb2389f17.. This was a source-only review; runtime occurrence and frequency
were not measured.

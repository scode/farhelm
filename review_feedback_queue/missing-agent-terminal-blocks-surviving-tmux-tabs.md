# Missing agent terminal blocks surviving tmux tabs

Reviewed commit: 53e2f0d96335108a7de295168d77928e992a55b7

## TLDR

After the agent terminal disappears, surviving older shell tabs can remain listed while opening or closing them is
refused.

## Details

Confidence: source-confirmed, subject to the qualifications below. Suggested bucket: high.

Source location:
`crates/farhelm-supervisor/src/service/terminals.rs:893-948 at 53e2f0d96335108a7de295168d77928e992a55b7`.

Source assessment (read with the later corrections below) (review record g018/d1, event 23):

Reviewed commit 53e2f0d96335108a7de295168d77928e992a55b7. TLDR: the terminal-loss fix makes surviving old terminal tabs
visible, but selecting or closing one still fails because the agent's terminal is missing; their work remains running
and inaccessible through those actions. Confidence confirmed source mechanism; no runtime reproduction. Assigned patch
changes status::entry_info at 529-547 and core::session_tabs to discover old tmux tabs even when entry.terminal=None,
using session_tmux_name and no agent pane. Necessary resolver context was not updated: resolve_terminal_inner handles
host tabs first at terminals.rs:885-891, then requires entry.terminal at :918-941 BEFORE matching Agent versus Tab. A
surviving tmux Tab selector therefore returns NotFound with the missing-agent-terminal message instead of rediscovering
its tab. Both resolve_terminal and resolve_terminal_for_close use this same funnel (:759-778). Initial source search
identifies attach at handlers.rs:1484 and close at core.rs:15874 as callers; full caller/temporal checks follow.
Concrete supported state is a migrated session with independent old tmux tabs whose host agent terminal ends, then
reload leaves no agent terminal; the assigned patch and its regression deliberately cover such a None-plus-live-tab
state. Terminal tabs are independent of the agent per SPEC.md Lifecycle operations and SPEC_impl.md Tabs in hosts, and
the resolver's own docs :879-882 acknowledge that a tab outlives its agent terminal. Suggested bucket high: loss of
access and close controls for still-running tabs after terminal loss; no established loss of user work, so not highest.
proposed_drop false. possible_cover: supplied g029:r2 and g030:f6 do not discuss this lookup gate; parent handles
further coverage. Caveat: lookup gate predates the patch; this is an incomplete edge of the assigned visibility repair,
not introduced source logic. Fix should apply the missing-terminal refusal to Agent only, while resolving old tabs from
the derived session tmux name with no recorded agent pane exactly as listing does. Add a regression that attaches and
closes the surviving listed tab with entry.terminal=None, rather than checking list/guard alone.

Later qualification (review record g018/d1, event 24):

Supplement to d1's complete preceding finding. Full callers confirm the source route: handlers.rs:1444-1462 takes tab
lifecycle ownership, :1463 fetches the current None-terminal entry, and :1484-1488 forwards the resolver's
missing-terminal NotFound before attach setup. core.rs:15862-15874 likewise loads the current entry and fails
explicit/automatic close at resolve_terminal_for_close before any window cleanup. The close code itself already
tolerates no recorded agent pane at :15889-15892, so the early shared resolver gate is the blocker. git log on
service/terminals.rs in 53e2f0d96335108a7de295168d77928e992a55b7..8a8c9579e04299410652f0e4681975a7198dfc7b returns no
commits: no in-range later repair. Confidence confirmed source-only, suggested bucket high, proposed_drop false,
possible_cover only parent-owned unknown coverage. This is NEW discovery d1, distinct from c4's fixed visibility/consent
mechanism and from supplied neighbor records. No claim of actual runtime occurrence or new data loss.

Later qualification (review record g018/d1, event 26):

Final discovery qualification supplements d1's earlier complete mechanism and caller evidence. Read full SPEC.md Session
view terminal-tab contract at this reviewed hash, lines 1711-1744: tabs survive supervisor restarts, can be closed
individually, and exited tab shells are automatically reaped. The assigned repair's actual None-plus-live-old-tab
fixture proves the intended data shape and verifies list/guard only; it does not exercise attach/close. Existing
independent old tmux tabs beside a migrated host agent are supported, so an ordinary agent-host loss plus routine
supervisor reload can leave that shape without direct user manipulation of the private tmux server. The resolver's
comment :899-901 still assumes no agent terminal means no tmux session, the exact assumption removed from the changed
listing/guard code. No later resolver change through pinned tip. No new high-risk consent or process-kill route is
claimed: c4 remains the distinct repaired highest-capable issue. Confidence confirmed by source, high, proposed_drop
false; possible_cover parent-owned and not searched. Search also inspected restart tab-preservation preflight, fixture
parser use, descriptor flag test isolation, and stand-in child ownership/drop boundaries; these produced no further
finding. New verified discovery IDs: d1 only. New unverified-after-investigation finding IDs: none. Existing l2/l3 were
not re-investigated with unchanged unavailable evidence.

Review provenance: trailing review run `dc410e54-098d-481c-be02-f2f9b4826f83`, canonical `g018/d1`. The retained private
log preserves earlier wording and its corrections. This record does not claim runtime reproduction.

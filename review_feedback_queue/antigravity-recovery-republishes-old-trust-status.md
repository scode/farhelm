# A recovered Antigravity terminal can republish its old trust question before sampling

Reviewed commit: 470e9e6ceb01d87cfc776fa7dab8bae04ae5d24e

## TLDR

A recovered terminal can initially show Antigravity’s old trust status and date the first fresh screen as new activity.

## Details

the assigned reader anchors Waiting and sets the shared current trust flag through agent_kind/screen_reader.rs:480–484
and service/ticker.rs:465–478, :1364–1393. On terminal re-adoption, service/core/host_handover.rs:1633–1635, :1680–1706
installs the recovered host but clones the old run and sample. hosts/terminal.rs:1854–1860 and :1918–1929 allow adoption
to return a live runtime before replay finishes. service/ticker.rs:1319–1335 withholds sampling until catch-up.
service/listing.rs:88–96 permits a list request without sampling. service/status.rs:195–214, :287–297, :339–348,
:417–420 projects the retained Waiting and trust flag for the new live host. The previous unreadable host masked the
trust flag through Unknown or a terminal outcome. The first fresh reading also takes its predecessor from the old
sample's transition-ready state at service/ticker.rs:607–627, so a recovered non-trust screen can cause
Waiting-to-Running to date activity and reorder the row at :287–319, :1408–1413. SPEC_impl.md:3538–3546 requires
resetting the current flag on failed capture, and :4070–4076 makes the first successful post-failure sample a baseline.
No later in-range commit touched the relevant sampler, status, or re-adoption files. Invalidate current screen evidence
and its transition baseline at re-adoption while preserving durable timestamps and pending writes.

Confidence: confirmed by inspection. Actually seeing the transient stale reply requires listing before a fresh sample.

Provenance: trailing review d3e8a4e8-dfeb-4080-a7b4-080b9065a167, `agScreen/l6`.

# The blocked alert's dependence on the master ntfy switch is untested

Reviewed commit: 721dd96fe7315659b661faa0ede8656d5f4192a1

## TLDR

SPEC promises that the Agent is blocked alert goes out only with ntfy alerts enabled "as well". Nothing in the test
suite would notice if a change let blocked alerts (which carry an agent's free-text reason to the ntfy server) go out
while the user has ntfy alerts switched off, or if blocked alerts were accidentally made to depend on the waiting
switch.

## Details

Settings::sends_blocked_alerts (crates/farhelm-proto/src/ntfy.rs:150-152) is
`enabled && notify_blocked && !topic.is_empty()`, and the sender gates Blocked alerts on it
(crates/farhelm-helm/src/ntfy.rs:328-334, 386-389). The proto unit test
`waiting_alerts_need_the_master_switch_the_waiting_switch_and_a_topic` (crates/farhelm-proto/src/ntfy.rs:286-313) covers
only sends_waiting_alerts. The helm test `blocked_alerts_follow_their_switch_and_carry_the_reason`
(crates/farhelm-helm/src/ntfy_tests.rs:920-968) saves with the master switch on in both phases and the waiting switch on
in both phases, so it pins only the blocked switch. Dropping `self.enabled &&` from sends_blocked_alerts, or writing
`self.notify_waiting` where `self.notify_blocked` belongs in one conjunct, would pass every test I found (grep for
notify_blocked/enqueue_blocked/sends_blocked_alerts in tests). The master-off case is reachable: Config::saved
(crates/farhelm-helm/src/ntfy.rs:45-104) lets a user save enabled=false with notify_blocked=true and a topic, and agents
can run `farhelm agent notify` regardless (it records the bell notice whatever the switches say), so the regression
would post agent reasons to a topic the user believes is off. Fix: extend the proto table test to sends_blocked_alerts
(master off, blocked off, empty topic all false; waiting off still true), or add a sender case with the master off.

### Triage notes

- Recorded bucket: `other`.
- Suggested bucket (reviewer): other (test gap on a privacy-relevant gate; current code is correct).
- Confidence: confirmed (the gap; the code is correct today).
- possible_cover: none known
- proposed_drop: none

### Provenance

Trailing code review of `dbb153ec94149a598a747af8ab306a3df0514641..b2d540a8dacfb94140e5eb0927d78e447ce46111` (run
`ef8bcf48-a7d4-4d80-9e04-457b0ff30acc`), Claude Code `claude-opus-5-5` at high effort under persistent-review.

- g19/f2 (finding), group g19 "ntfy waiting alerts: helm delivery", findings log line 2.

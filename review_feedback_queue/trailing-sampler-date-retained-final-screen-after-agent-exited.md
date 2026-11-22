# Sampler can date a retained final screen after the agent exited

Reviewed commit: 44137d0659d28b9cadf90a8ffe96a65930035aad

## TLDR

If an agent exits during another session's sample processing, its retained work screen can refresh its age and unseen
marker after the exit.

## Details

**other.** Source inspection. Main reference `crates/farhelm-supervisor/src/service/ticker.rs:1355`.

sample_pass checks caught-up live evidence once while collecting the whole rotation (ticker.rs:1311–1329) and later
awaits activity or work-start persistence for individual entries (1400–1404). The host runtime's Exited state explicitly
retains the final screen (hosts/terminal.rs:490–495,2236–2246). A later entry still reads this screen at
ticker.rs:1355–1368. The status comparison uses status.rs:287–298, which only projects the activity cell and assumes its
caller established liveness. The loop neither rechecks the host nor uses that comparison to gate writes (1376–1404). A
retained anchored Working frame dates even unchanged if a baseline exists (286–294). Crossing the quantum updates the
age/unseen value (1566–1586). SPEC.md:3065–3068 dates when an agent is seen working, while the corrected
SPEC_impl.md:4003–4009,4068–4075 describes sampling live terminals. Existing ticker.rs:4615–4667 checks a terminal
already ended before a later pass, leaving the exit-between-selection-and-read window untested. Revalidate at each
observation, pairing the screen and liveness consistently. Bucket: other (occasional misleading recency, with ended
status governed independently). Confidence: source establishes the window and retained-frame data path. No timed
observation of the interleaving. Possible coverage: none. L20 addresses valid historical observations.

Review evidence is retained under run `4d6e9e3b-26c2-4fd0-aeb6-997bfc824377` as `recovery_contract/l22`.

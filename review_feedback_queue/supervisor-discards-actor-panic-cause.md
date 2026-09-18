# Connection actor panic loses its cause in the visible record

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When a host's connection handler crashes, the log and the host's status show only a generic "panicked" message — the
actual reason is nowhere in the visible record.

## Details

Source: pre-pr-review-swarm, area helm-fleet, 2026-09-17. Confidence: possible, low. Coordinator verified, with a stderr
caveat.

When the supervised actor task ends panicked,
`match actor_task.await { ... Err(_) => "the connection actor panicked;
..." }` (manager.rs:1626-1633) drops the
`JoinError` — panic payload/message discarded; the warn log and the `Retired.reason` carry only the static string.
Scoped honestly: with no custom panic hook anywhere in the workspace (coordinator grepped), the std default hook still
prints the payload to stderr (journal under systemd) — so the cause survives OUTSIDE the structured record, but the
operator-facing surfaces (unified log line, API/UI-visible `Retired` reason) carry nothing diagnosable, on the path
explicitly documented as the never-silent one ("this entry is not being attempted" is where the evidence matters most).
Concrete trigger (actor panic) with a concrete diagnostic loss at the moment of failure; 3-line fix. Possible, low.

Suggested fix: capture `error.into_panic()` (or at least the `JoinError` Display) into the log line and, bounded via
`peer_text`-style capping, into the reason.

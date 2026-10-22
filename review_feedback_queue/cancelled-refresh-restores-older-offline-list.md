# Cancelled refresh restores an older offline list

Reviewed commit: 9d266e28595e8c2c07b378174f35eedf84258a70

## TLDR

An older cancelled refresh can overwrite a newer cached session list. If the host remains unavailable afterward, a
created session can stay missing or a deleted one can reappear in the offline view across helm restarts. The actual
sessions and user work are not lost by this mechanism.

## Details

A reconnect nudge, including Retry, can cancel refresh (`crates/farhelm-helm/src/manager.rs:4097–4103`). Refresh takes
the cache lock and checks the seed epoch before submitting a database write (`4422–4452`). The database wrapper submits
blocking work behind a mutex (`farhelm-supervisor/src/db.rs:128–143`); cancellation of its awaiting future does not undo
an already running closure. The async cache lock can therefore be released while old work still waits for the database.
A newer write may commit before that surviving old closure. This adverse ordering is inferred, not observed.

The transaction verifies installation identity but has no epoch check (`farhelm-helm/src/store.rs:6894–6914`), then
replaces the list from its captured entries (`6952–7009`). For the same installation, it can undo a newer cached result.
Offline detail uses that durable cache and marks it stale (`sessions.rs:2593–2625`); a helm restart cannot obtain a
correcting list while the host remains unavailable.

A successful, non-discarded refresh committing after all stale writes repairs membership, metadata and truncation,
subject to intentional Unknown-status retention. An earlier successful refresh can itself be overwritten. Pins, seen
state and notification marks reference the host rather than cache rows (`store.rs:427–456`), so cache replacement does
not cascade into those choices. Offline lifecycle routing refuses without a client (`sessions.rs:804–826`). No
wrong-host operation, security consequence or irrecoverable state is established. If corrected, the transaction boundary
needs an ordering guarantee that survives cancellation of its async caller.

Suggested bucket: **other**. Confidence: **likely**, with the stale-write ordering and subsequent outage unverified.
Source inspection only; no reproduction is requested or authorized by this review.

Possible cover: `TRIAGE_OUTCOMES.md:2430–2442`, `cancelled-refresh-overwrites-seed.md`, is the same mechanism,
previously dismissed without re-verification as brief/view-local. Preserve that disposition for the temporary case; this
finding retains only the prolonged durable offline consequence. `SPEC_impl.md:3727–3738` makes mutation caching best
effort but also says older refreshes decline to erase it. The rare-glitch filter excludes durable wrong state. The
human-race filter might apply if the only reachable cancellation requires a click in its bounded window, but that timing
premise is unverified. No exact Planned, BUGS or open queue item was found. Independent assessment retained the
remainder. Proposed drop: none for the offline extension.

Provenance: persistent reviewer `A20-cache/A20-cache-f1`, independent `static-dispositions-11`, parent source and
coverage verification. This is an extension of the prior disposition's scope, not a new cancellation root cause.

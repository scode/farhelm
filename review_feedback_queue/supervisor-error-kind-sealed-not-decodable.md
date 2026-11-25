# Supervisor error-kind on-disk vocabulary does not round-trip: encoder emits "sealed", decoder rejects it

Reviewed commit: 423770e10621b745ca94ab56e3c30595c874074a

## TLDR

Latent. If a create reservation were ever settled as a failure carrying the "sealed" error kind, that reservation row
could never be read back — replaying the same create intent would error instead of replaying the original refusal. No
current writer produces it, so there is no established user impact today.

## Details

**other.** Source inspection; commissioner confirmed both functions and that no current writer emits Sealed into a
reservation. Correctness/robustness; no established user-visible consequence. Not highest: an undecodable reservation
row would surface as a create-replay error, not loss of data/credentials/processes. It would move up only if a concrete
writer emitting Sealed were shown AND the resulting failure crossed into data loss or security — neither established.

`crates/farhelm-supervisor/src/store.rs`: the encoder `error_kind_column` is exhaustive over `farhelm_proto::ErrorKind`
(compiler-enforced) and spells `K::Sealed => "sealed"` at line 1088. Its documented inverse `error_kind_from_column`
(lines 1095-1109) accepts the other nine spellings but has NO "sealed" arm, so its catch-all
`other => anyhow::bail!("reservation row has unrecognized error kind …")` at 1107 fires.
`ReservationOutcome::from_columns` (failed branch) calls this decoder, so a `failed` reservation row with
error_kind='sealed' would fail to decode in `read_reservation`. The encoder comments describe Unavailable, Timeout,
TakenOver and Sealed alike as "never written today / never a create outcome", yet the decoder DOES carry arms for
unavailable/timeout/taken_over and only omits sealed — the author's "spell both directions for the no-panic kinds"
pattern was followed for three of the four and missed for Sealed.

Latent premise (commissioner-verified): the Sealed usages in service/core.rs (8872, 13945) and service/handlers.rs
(11603, 11698) are attach/tab refusals and request errors, not create-reservation outcomes; the encoder comment ("A
sealed supervisor's create refusal records nothing, so this is never written today") holds — no writer emits Sealed into
a reservation row today.

Verify: by inspection the round-trip error_kind_from_column(error_kind_column(k)) == k fails for k = Sealed. Fix shape:
add `"sealed" => K::Sealed` to error_kind_from_column so the vocabulary round-trips, matching the encoder and the three
sibling kinds. Low risk.

Reviewer metadata (area 9 chunk F, supervisor store first third): suggested bucket: other; confidence: encoder/decoder
mismatch confirmed, user-visible consequence latent (no writer emits Sealed); proposed_drop: none; possible_cover: the
in-code "never written today" comment is the latency basis, not a queue item. Commissioner validation: both functions
confirmed; Sealed writers checked (none reservation); coverage searched (queue, TODO.md, BUGS.md, TRIAGE_OUTCOMES.md,
plans/queue) — no match.

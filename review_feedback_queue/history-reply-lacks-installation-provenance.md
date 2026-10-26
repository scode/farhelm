# A history reply can be labeled as another installation

Reviewed commit: 4ae55cc50dafcae6836abe835939ca79034cbdf7

## TLDR

If a host changes installations and changes back while the GUI misses the intermediate snapshot, history from the
intermediate installation can become the current host's frozen folder default. Final launch still targets the current
machine.

## Details

Suggested bucket: **other**. Confidence: Source-only assessment; the established mechanism, contract ambiguity and
verification limits are preserved below. No reproduction was performed.

Source location: `crates/farhelm-ui/src/list/create_form.rs:1212-1213`. Locations and revisions in the assessment below
identify the reviewed source; later main is outside this review's pinned range.

Reviewed commit: 4ae55cc50dafcae6836abe835939ca79034cbdf7. TLDR: after another client retargets a host entry away and
back, an open launcher can use the intervening installation's recent folder as the restored installation's default; an
up-to-date create connection check does not repair that value. Confirmed source path: api.rs:3959-3970 sends only host
ID and LaunchHistory at 158-166 returns no identity. Helm sessions.rs:554-585,1232-1264 chooses the currently connected
installation and reads its partition (store.rs:7807-7878). UI create_form.rs:1308-1318 labels the reply with its
captured target. The assigned new default code trusts that label at 1212-1213, reads the newest ordinary cwd at
1102-1107, and settles the decision at 1221-1223. A supported sequence keeps the foreground client's target at A while
another client retargets/adopts the same row to B, records an ordinary GUI launch into a B folder, and the history GET
is serviced on B; then retarget/adopt back to A before delivering the saved B reply, without this client consuming an
intermediate B registry snapshot. set_destination preserves the row (helm hosts.rs:865-913), and adopt_identity allows B
then A on that row while purging retired partitions (store.rs:6864-6938). The already-read B response survives that
purge. host_incarnation returns to the identical A fingerprint because it uses ID/identity/destination/install fields
and omits the connection token (UI hosts.rs:528-567). A final A registry refresh can therefore update
HostOption.connection while leaving CreateTarget equal to captured A (shared.rs:623-643; view.rs:1304-1323). B's reply
passes the equality check and is promoted/frozen (create_form.rs:2775-2806,2853-2884). Follow-up reads are coalesced,
not cancellation of the in-flight read (reader.rs:444-456,644-679), and later correct history does not reseed that
decided default. On a later create, the current A connection token is read at create_form.rs:4721 and sent at 5021-5029;
it passes helm sessions.rs:1496 and precondition.rs:106-117 against A. If the wrong folder exists on A, ordinary folder
admission need not refuse it. A host-only quick-launch template can also follow the ordinary auto-submit path after
pending/unread clear (4154-4189); no security or user-work-loss consequence is inferred from that fact. Suggested
bucket: other, as requested for the established wrong-history/default contract effect. Confidence: confirmed source
mechanism conditional on the stated asynchronous ordering. Caveats: no reproduction or runtime measurement; the history
response must finish within api.rs:1623's ten-second read deadline, B needs an eligible ordinary launch row when read,
and the client must omit intermediate target snapshots. Actual frequency is unmeasured, not a missing SOURCE premise. No
cross-host dispatch, exfiltration, destructive action or user-work loss is established. Pinned-tip api.rs:3989-4001 and
create_form.rs:1381-1393 retain the same unbound reply interface. possible_cover: none found. Pinned
SPEC.md:235,565-576,838-841 keeps defaults/history installation-bound; SPEC_impl.md:3548-3552 describes correct target
retention but does not accept response relabeling. TODO Planned is empty; its Maybe-later identity-simplification idea
is neither planned coverage nor this response route. Targeted BUGS/index/ledger searches found no matching item.
TRIAGE_OUTCOMES.md:4696-4725 concerns a different subsecond Adopt/publication race. No filter applies to the whole
reported route: it does not require a person to win a subsecond click window, and accepted creation/hidden quick launch
can act on the wrong folder rather than only briefly mispaint a diagnostic. proposed_drop: none. This is distinct from
a010/g018 canonical-path normalization and f1's correctly-tagged stale-row rejection. A remedy must bind the history
response to the installation actually queried instead of trusting a client-created target tag.

Provenance: `g242/f3`, `a030/g242/f1`. These are reviewer-local identities, not additional findings. Proposed drop: none
for the retained remainder.

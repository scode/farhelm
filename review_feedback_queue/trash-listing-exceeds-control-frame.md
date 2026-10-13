# A large trash listing disables all cleanup from the app

Reviewed commit: d2f4508e01366c15ade8a84dff988a6400ef16ba

## TLDR

After enough managed checkouts accumulate in the trash, Farhelm can stop showing or deleting any of them. Long checkout
paths make this happen with fewer entries; ordinary app cleanup cannot recover it.

## Details

Suggested bucket: `high`. Confidence: confirmed size-budget mechanism by source inspection and serialized-size
calculation; a large on-disk archive collection was not constructed. Source: adversarial checkout reviewer. Proposed
drop: none. The trigger is unusual: many retained archives, especially with long paths. The reviewer considers `other`
reasonable if triage judges that scale too remote. This does not claim loss of archived files or a connection crash.

`crates/farhelm-supervisor/src/checkout_trash.rs:277–331` collects every retired archive into one listing, and
`crates/farhelm-supervisor/src/working_copies.rs:922–933` reads the registry without a limit. The handler sends the
entire listing in one reply (`service/handlers.rs:2407–2426`). The control-frame ceiling is 8 MiB
(`crates/farhelm-proto/src/lib.rs:415`); `service/connection.rs:951–986` replaces an oversized reply with an error.

The UI requires a successful full listing before producing any deletion target
(`crates/farhelm-ui/src/list/trash.rs:85–100`). Single-entry deletion calls the same target builder (`123–135`), so it
cannot trim the collection back under the ceiling. Both count reads and measured dialog reads carry every entry. An
isolated calculation using the actual JSON fields and frame overhead produced 8,910,072 bytes for 2,200 entries, with
checkout paths at most 3,848 bytes and archive paths at most 3,897 bytes. Those fit Linux's admission and path limits
(`working_copies.rs:76–87`) and remain below the 10,000-ID cleanup limit. That calculation did not exercise product
listing or create archive directories. Ordinary short paths require many more archives. Retired archives accumulate
across session lifetimes, not just concurrently open sessions. Introduced by `a1b1f6137c34f28f812855152755e99ba898e9dc`.

Verify by constructing an owned archive registry whose encoded listing exceeds the frame budget, establishing that its
paths fit the platform limit, then reading trash and attempting single-entry cleanup. The list path needs a bounded or
paged representation that preserves an ordinary cleanup route; merely returning a clearer error leaves the lockout.

Possible cover: SPEC_impl.md:1800 accepts refusing a host above the 10,000-ID **batch** limit. This overflow can occur
below that limit and disables listing and individual deletion too. SPEC.md's concurrent-session scale does not cap the
historical archive collection. No exact Planned item, known bug, queue item or prior triage decision was found.

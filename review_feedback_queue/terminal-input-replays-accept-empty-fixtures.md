# Terminal input replays can pass with empty fixtures

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

The terminal key and mouse compatibility replays can pass without running their assertions when their fixture tables are
empty.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

### The key-policy replay accepts an empty case table

The fixture reader can return no case rows and the test only prints the resulting zero count. Pin the count of replayed
cases so an emptied or truncated corpus fails.

Reviewed locations: `crates/gui/farhelm-input-mapping/src/tests.rs:155-196`. Haiku out-T8.md §1. Suggested bucket
**other**.

### The mouse replay accepts an empty case table

An empty mouse fixture skips every admission and byte assertion. Pin the fixture row count before replaying it.

Reviewed locations: `crates/gui/farhelm-input-mapping/src/tests.rs:935-1006`. Haiku out-T8.md §1. Suggested bucket
**other**.

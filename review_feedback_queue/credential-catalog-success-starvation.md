# Unrelated updates can prevent a credential catalog from publishing

Reviewed commit: 49ce8cb76e9f4c841e15122a89264c7c908171dc

## TLDR

If unrelated fleet updates arrive faster than credential-list replies, Credentials can remain at Loading credentials
with Add disabled even though every read succeeds. An already loaded list can instead stay stale. A quiet interval lets
it recover; practical frequency has not been measured.

## Details

Suggested bucket: **other**. Confidence: confirmed conditional scheduling route from source; no observed production
outage or runtime reproduction. Introduced by `3126f25e98b5468b70cdf6ae2b683365c6699d9d` and extracted into the shared
catalog hook by `64734bf4b6034a720b8d15d210cc4db705f8a966`.

`crates/farhelm-ui/src/credentials_settings.rs:92–108` increments a generation and spawns a read for every reload;
`98–99` discards a reply if any later read started. The feed callback invokes reload directly (`110–111`). Thus each
notice can invalidate the successful in-flight result before the next notice invalidates its successor. Initial rows
remain absent and Add remains disabled (`175–178`); last-good rows remain unchanged after loading.

The feed hook explicitly does not await the consumer's read (`feed.rs:257–283,289–315`). Unrelated session hints and
cache changes can produce notices (`events.rs:206–249`; `manager.rs:4584–4596,4644–4645`) independently of REST
completion. No second GUI or credential mutation is required. The existing `SurfaceReader` mechanism coalesces demand
and bounds in-flight work (`reader.rs:24–30,74–85`), but this hook does not use it. A ten-second per-read timeout
(`api.rs:2042`) bounds fixed-rate overlap; unbounded task accumulation is not claimed.

SPEC.md, Credentials, `1947–1961` requires the catalog and reference refresh. SPEC_impl.md, Named credential catalog,
`5976–5982` requires shared metadata readers and last-good catalogs. Successful discarded replies produce neither an
error nor Retry; explicit retry after a failed read is already available in both consumers. Retry does not remove this
starvation while notices continue to outrun responses.

Use coalesced feed demand with a trailing read after the in-flight read, keeping generation fences for actual target or
snapshot changes. Correct hidden-selection rendering after publication does not establish that publication makes
progress; the recipe reviewers reconciled that distinction. No exact acceptance, planned repair, queue, ledger or filter
coverage was found. The duration is conditional on continuing notices, rather than necessarily a brief display corrected
by its next refresh.

Provenance: run `4ca8e296-4f73-4429-a34b-031af1582085`, `g004/g004-catalog-flight`, with scope clarification in
`g007/g007-l003` and `g008/g008-c003`. No credential loss or security route is claimed. Proposed drop: none.

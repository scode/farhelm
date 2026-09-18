# Cached session detail shows a row the list hides as poison

Reviewed commit: 19edfdf087b1443dae940d5a16ba8c84e882ccfb

## TLDR

When a session's cached record is corrupted, the session list correctly hides it but opening that session's detail view
still shows it — with a timestamp the database itself contradicts.

## Details

Source: pre-pr-review-swarm, area stores, 2026-09-17. Confidence: possible — in-band writers always write column and
payload from the same struct, so only a hand edit or downgrade produces the disagreement (the db file is an explicit
trust boundary of this module).

The page scan (`cached_rows`, store.rs:5353-5385) treats a row whose payload `id` OR `created_at` disagrees with its
filing columns as poison (skips + warns, store.rs:5357). The single-session stale-detail read (`cached_session`,
store.rs:5404, behind `GET /api/sessions/{id}` on a down host) selects only `info_json` (store.rs:5415) and checks the
`id` alone (store.rs:5428) — while its comment claims "the same column/payload contradiction the page scan treats as
poison, and the same answer" (store.rs:5425-5427), which is false for `created_at`. A corrupted row is thus hidden from
the list yet shown by the detail view with a contradicted timestamp.

Suggested fix: select `created_at` and extend the guard to `info.id != session_id || info.created_at != created_at` with
the same warning.

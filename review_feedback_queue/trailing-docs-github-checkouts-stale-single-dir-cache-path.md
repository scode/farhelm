# docs/github-checkouts.md still documents the pre-redesign single-directory repository cache path

Reviewed commit: 1d711f95529c1d9954ad6fed1013c4f923e42240

## TLDR

The repository's managed-checkouts guide still says each host stores a repository's cache at
`<supervisor-state-dir>/repo-cache/<owner>/<name>.git`, even though Farhelm now stores caches in separate
`<name>.git:gen-<uuid>` directories selected by `<name>.git:current`. Anyone following that guide to inspect or delete a
cache by hand will not find the path it names.

## Details

**other.** Source inspection. Main reference `docs/github-checkouts.md:93`.

Commit `8dccd62785cf244c7b890ef261c677fb47d0cb44` changed the physical cache layout from
`<state_dir>/repo-cache/<owner>/<name>.git` to `<name>.git:gen-<uuid>` with selector `<name>.git:current`
(`crates/farhelm-supervisor/src/repo_cache.rs:39-48`, `SPEC_impl.md:3626-3634`). Commit
`1d711f95529c1d9954ad6fed1013c4f923e42240` updated `website/src/content/docs/docs/using/start-a-session.mdx:305-307` to
describe the new layout, but `docs/github-checkouts.md:93` was missed and still states:
`The cache is <supervisor-state-dir>/repo-cache/<owner>/<name>.git, outside the working-copy root.` Verification: read
`docs/github-checkouts.md:93-97` and compare against `website/src/content/docs/docs/using/start-a-session.mdx:304-313`
and `SPEC_impl.md:3626-3661`. Fix: update `docs/github-checkouts.md:93-97` to match the `<name>.git:gen-<uuid>` and
`<name>.git:current` layout (and note that removing the cache directory or selector forces a fresh download while
leaving `.lock` in place).

Reviewer metadata (g31_repo_cache_lock_redesign/f2): suggested bucket: other (stale documentation with no security or
data-loss impact).; confidence: confirmed (source analysis).; possible cover: none.

Review evidence is retained under run `746bd2ca-c078-44c7-90d6-305e2a679dca` as `g31_repo_cache_lock_redesign/f2`.

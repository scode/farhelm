# A stale preview lock may identify a later server in the same checkout

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

A stale preview lock may identify a later server in the same checkout.

## Details

`F42 / COR-PREVIEW-LOCK-IDENTITY` — **possible** — `website/scripts/preview.sh:118` — A stale preview lock may identify
a later server in the same checkout

This possible finding concerns a stale preview lock, the file Astro uses to remember its background server. Before
asking Astro to stop that server, the preview script checks that the recorded process ID is running from the expected
website directory, comparing the directory's device and inode. That rejects a recycled ID belonging to another checkout,
but cannot distinguish two successive Astro processes running from the same directory.

If a stale background lock survives and its number later belongs to a maintainer's foreground Astro server in that
checkout, this check could allow the stop request to reach that foreground server. Record and validate a process start
identity, and refuse a stop when ownership cannot be established. Proposed bucket: highest. No possible cover was
identified in the input, but the route remains uncertain.

Restater note: Astro's stop implementation is not installed in the pinned checkout, so its behavior could not be
verified. The script explicitly ignores locks with `background: false`, and a normal foreground Astro invocation may
rewrite the lock that way. That may prevent the proposed scenario entirely; no route preserving the stale background
lock through such a launch was confirmed.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **possible**. Suggested queue bucket: **highest**.

Originating reviewers and passes: `auto_data p1`.

Possible cover recorded during collection: none identified.

Collection caveats: Astrostop internals unavailable; foreground invocation normally rewrites lock backgroundfalse may
refute. Auditor uncertain, no confirmed route.

## Filed reviewer metadata

- `auto_data p1`: confidence as filed: possible; materially open premise is that a stale background lock can still name
  a later Astro process in the same directory when Astro stop evaluates it. Suggested bucket as filed: highest (possible
  termination of foreground work). Confidence: possible; materially open premise is that a stale background lock can
  still name a later Astro process in the same directory when Astro stop evaluates it.

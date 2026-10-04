# Preview startup can report an unrelated server as the docs preview

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Preview startup can report an unrelated server as the docs preview.

## Details

`F54 / COR-PREVIEW-READY-IDENTITY` — **definite** — `website/scripts/preview.sh:76` — Preview startup can report an
unrelated server as the docs preview

The docs preview reports success when the fixed port's listener has the website directory as its working directory. That
device-and-inode comparison establishes the checkout, but does not establish that the listener is Astro or that it is
the background preview this script manages. An unrelated HTTP server launched from the same website directory therefore
satisfies the check.

On the initial check, the script can immediately print the docs URL and exit successfully without starting Astro. The
same weakness affects the final check after startup: a competing same-directory listener can take the fixed port while
Astro moves to another port, and the script can still report the fixed address as ready. The result is a link to the
wrong service presented as a working docs preview.

Require the actual Astro background instance's identity, including its lock and relationship to the listener, before and
after startup. Add a negative case with a non-Astro listener from the same website directory. This conditional false
success is established from the source; no server was run, and no destructive consequence is claimed. Proposed bucket:
other. No possible cover was identified.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **other**.

Originating reviewers and passes: `auto_systems p2`.

Possible cover recorded during collection: none identified.

Collection caveats: No server run; conditional samecwdnonAstro listener; no destructive consequence.

## Filed reviewer metadata

- `auto_systems p2`: confidence as filed: **definite / confirmed** from predicate and caller contract, conditional on an
  existing non-Astro listener whose cwd is the website directory. Suggested bucket as filed: **other** (wrong local
  preview link and false successful startup; no established data/process/security loss).

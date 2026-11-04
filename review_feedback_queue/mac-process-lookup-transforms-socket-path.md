# MacOS process lookup compares changed command text with the raw socket path

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Mac test cleanup can remove a stopped terminal host's live socket and directory under certain valid injected paths,
leaving the host unreachable for future cleanup.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed conditional
source defect for an injected path with repeated ASCII spaces or ps-escaped controls, plus a stopped/full-backlog Mac
host. No runtime reproduction or measured flake claimed.

Source locations: crates/farhelm-teststate/src/hosts.rs:299-309 (1eef3d64e36cd31b56e5b7e815fa3cc183b55637).

the wanted suffix at hosts.rs:299 retains the socket's exact spaces, while the entire ps row is split_whitespace and
joined with single spaces at 301-305. A socket argument under a root containing two consecutive spaces therefore cannot
match at 309. Public lib.rs:790-818 accepts injected roots without rejecting or rewriting internal spaces, and
supervisor hosts/spawn.rs:231-239 supplies the resulting absolute path directly as an argument. In the
stopped/full-backlog case, three seconds of macOS connection refusals invokes confirm_gone:215-221, which treats that
false negative as absence and unlinks the live socket; TestDir cleanup or the sweep can then remove the directory. The
conditional source route remains confirmed; it is not a measured test flake. Primary-source synthesis: Apple's official
adv_cmds-237 commit 6bed8737a34dbb54782a18f47dccf933a9967a12, ps/print.c:224-253 converts argument NUL separators to
spaces, 292-303 copies the result, 316-342 visualizes it with VIS_TAB|VIS_NL|VIS_NOSLASH, and 365-368 emits the full
last command column when unlimited; ps/ps.c:453-458,484-487 makes -ww and piped output unlimited. This supports the
repeated-ASCII-space finding. Correction to the original event: do not attribute tabs/newlines to the same split/rejoin
mechanism, because ps visualizes those control characters before this parser sees them; any mismatch for them has a
separate representation cause and is not claimed here. Fix: preserve internal command whitespace, or query
argument-preserving process metadata.

a raw tab/newline in the suffix cannot equal ps's visualized text. Both path-representation cases belong to this
lookup's requirement to compare a raw pathname with a lossless representation, and are not additional finding IDs. Fix:
use argument-preserving process metadata, or preserve internal spaces and correctly decode ps's visualized command text;
merely changing split_whitespace does not repair escaped controls.

Mac test cleanup can remove a stopped terminal host's live socket and directory under certain valid injected paths,
leaving the host unreachable for future cleanup. Primary trigger: a root containing repeated ASCII spaces. The wanted
suffix at hosts.rs:299 retains its raw pathname, while split_whitespace/join at 301-305 collapses the process row's
internal spaces, preventing a match at 309. Public lib.rs:790-818 permits injected roots, and the exact locked tempfile
3.27.0 implementation preserves them: src/lib.rs:621-629 forwards the root, src/util.rs:22-82 joins it with a random
basename, src/dir/imp/unix.rs:6-25 creates and stores that path. Cargo.lock at tip:6135-6145 pins tempfile 3.27.0; its
.cargo_vcs_info.json identifies source commit 5c8fa12eb584931b4f1bccfde87eb72fbfa7dc61. Supervisor
hosts/spawn.rs:231-239 supplies that absolute path directly as a socket argument. In the stopped/full-backlog case,
three seconds of macOS refusals invokes confirm_gone:215-221, which treats the false negative as absence, unlinks the
live socket, and allows TestDir/sweep removal. Primary-source synthesis confirms that Apple ps preserves internal ASCII
spaces: official adv_cmds-237 commit 6bed8737a34dbb54782a18f47dccf933a9967a12, ps/print.c:224-253,292-342,365-368; its
ps.c:453-458,484-487 makes -ww/piped output unlimited. Apple Libc commit 71bbe350ab79eef58113991d817ccc6165061a64,
gen/FreeBSD/vis.c:289-299,335-368,745-749 preserves ASCII spaces unless VIS_SP is set; ps does not set it.
Correction/precision for the original event's tab/newline examples: ps escapes those controls through VIS_TAB|VIS_NL
before this parser sees them, so their same false-negative outcome has a different representation cause, not
split/rejoin alone. The original f3 retains those examples only on that corrected

Possible coverage and caveats: none. Required coverage sources were searched in the initial pass and remain pinned at
the same tip; TODO Planned:23-25 is empty, SPEC_impl.md:2504-2515 requires retaining unconfirmed hosts, and 6795-6808
permits name-based ownership but not false-negative matching. TRIAGE_OUTCOMES.md:7370-7395 addresses different unquoted
fixture redirects. No exact plan, queue, BUGS or ledger match; persistent orphaning exceeds FILTER's rare
self-correcting diagnostic boundary.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g030/f3`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 1eef3d64e36cd31b56e5b7e815fa3cc183b55637;
verified present at inclusive tip ca9b0a19856dc136f63ad2d14f1076027edce1eb:hosts.rs:299-315 and lib.rs:790-818.. This
was a source-only review; runtime occurrence and frequency were not measured.

# Desktop smoke misparses custom run directories containing spaces

Reviewed commit: ca9b0a19856dc136f63ad2d14f1076027edce1eb

## TLDR

Choosing a short fresh DESKTOP_SMOKE_DIR containing a space makes the gate report that a healthy created session has no
live terminal.

## Details

Suggested bucket: **other**. Confidence: **source-confirmed defect or conditional mechanism**. confirmed by repository
and kernel source; no reproduction performed.

Source locations: 58094288259b0c29f74ea25936d4fe006c72128f:scripts/desktop-smoke.sh:245-246; unchanged at
ca9b0a19856dc136f63ad2d14f1076027edce1eb.

the override is accepted and quoted at 146-152. The socket path at 239-240 contains the space, but line.split() at 245
splits that pathname too and fields[7] at 246 can never equal it. The first main calls at 1451/1458 therefore fail.
Static primary-source check: Linux v6.12 resolves via annotated tag 06090c9b622a7e1f797e775db4c035e0d779b76e to
immutable kernel commit adc218676eef25575469234709c2d87185ca223a; net/unix/af_unix.c:3402-3446, especially 3424-3439,
prints sun_path bytes directly, including spaces, rather than escaping whitespace. Thus this is a source-established
parser defect, not a hypothetical scheduling flake. A bounded split preserving the final pathname column fixes this
cause. The later proof command at 378 and optional legacy input at 2190 also need shell quoting when custom whitespace
paths are repaired; these are subordinate follow-through, not a measured separate failure here because the earlier
parser currently stops the common flow.

scripts/desktop-smoke.sh:146-152 accepts DESKTOP_SMOKE_DIR and quotes it when creating the private run directory. The
newly introduced session_terminal_alive parses /proc/net/unix rows with unrestricted split() and compares fields[7] to
the full socket path: a path containing a space is split into multiple fields, so no listener matches (assigned
p0840.diff lines 51-60). The main flow fails immediately at lines 1451/1458 despite a live host. Independently,
drive_terminal interpolates proof into an unquoted shell redirection at 378, and the legacy typed command does likewise
at 2190; merely fixing the listener parser leaves the same custom path failing there.

Possible coverage and caveats: none found in required candidate-specific searches across SPEC.md, SPEC_impl.md, TODO
Planned (empty at tip:23-24), BUGS.md, unlanded plans/queue, full queue INDEX/matching desktop-smoke file and
TRIAGE_OUTCOMES.md. SPEC_impl.md:6795-6808 accepts hostile-local-account omissions and overlapping-command limitations,
not whitespace-path parsing failures. FILTER.md:24-42 does not clearly cover a deterministic run failure that repeats
for the selected path.

Provenance: private review run `37478d09-7e36-409b-8efa-b13d1f793dd5`; original identities `g121/f2`. Original
append-only events, revisions, withdrawals and independent judgments are retained in that run's catalogs. Locations
refer to the named reviewed revisions; the original revision context is 58094288259b0c29f74ea25936d4fe006c72128f. Tip
persistence: git diff to ca9b0a19856dc136f63ad2d14f1076027edce1eb is empty for scripts/desktop-smoke.sh, and tip lines
138-158,212-248 were read directly.. This was a source-only review; runtime occurrence and frequency were not measured.

# ASCII spaces remain invisible in host-identity approval labels

Reviewed commit: bd8d5d76439f8f36bbc2637b44d7ba4026b65675

## TLDR

ASCII spaces could make distinct installation identities hard to distinguish during approval.

## Details

F44 — **possible** — `crates/farhelm-ui/src/peer.rs:102` — ASCII spaces remain invisible in host-identity approval
labels

The identity renderer leaves ASCII spaces visually implicit while exact comparison still treats them as meaningful
bytes. A peer identity differing only by such spaces can therefore be difficult to distinguish on the adoption surface.
Connection freezing and explicit adoption remain required; no authorization bypass or mistaken approval was
demonstrated. Render identity whitespace explicitly, including ASCII spaces, while preserving raw values for exact
comparison and submission.

## Evidence and triage context

- crates/farhelm-ui/src/peer.rs:98–112 preserves ASCII characters that are not presentation-unsafe. Lines 63–84 preserve
  spaces when the value also contains visible characters.
- crates/farhelm-proto/src/text.rs:53–75 does not classify U+0020 as presentation-unsafe.
- crates/farhelm-proto/src/lib.rs:2373 represents host_identity as Option<String>.
  crates/farhelm-proto/src/io.rs:552–555 checks identity length, not UUID shape or whitespace; SPEC_impl.md:1178–1182
  explicitly requires opaque, length-only admission.
- crates/farhelm-helm/src/store.rs:5185–5193 compares recorded and reported identities exactly. An otherwise identical
  identity with trailing spaces produces Mismatch.
- crates/farhelm-helm/src/manager.rs:3756–3767 returns the mismatch rather than connecting the peer.
- crates/farhelm-ui/src/hosts.rs:592–597 renders both mismatch identities through display_identity. Lines 2676–2679
  construct the Adopt label through that renderer; lines 3144–3158 render the label while submitting the raw reported
  string.
- crates/farhelm-ui/src/hosts.rs:1225–1238 forwards that raw identity to adopt_host.
  crates/farhelm-ui/assets/app.css:2257–2265 supplies ordinary flex/text layout for the label rather than visible
  whitespace notation.

Retained confidence: **possible**. Suggested bucket: **highest**.

Possible cover:

- {"basis": "TRIAGE_OUTCOMES.md:2333–2357, display-peer-misses-invisible-chars.md", "comparison": "The consequence and
  approval surface overlap, but the recorded trigger and remedy concern non-ASCII invisible characters, with completion
  criteria specifically requiring all non-ASCII to be escaped. ASCII spaces are outside that explicit scope. The
  execution is recorded complete, so it cannot silently cover this remainder."}
- {"basis": "SPEC_impl.md:1178–1182", "comparison": "Opaque identity admission explains why the string is accepted. It
  does not accept ambiguous presentation when asking the user to adopt it."}
- {"basis": "review_feedback_queue/FILTER.md:36–42", "comparison": "A possible trust-boundary consequence excludes the
  rare-diagnostic filter even though ordinary supervisors mint UUIDs without spaces."}

Caveats:

- The mismatch still freezes the connection and adoption requires an explicit click.
- No authorization bypass, credential exposure, or loss of user-owned work was demonstrated.
- No browser reproduction was performed; the significance of the visual ambiguity to an actual approval decision remains
  unverified.
- The highest bucket is conservative because the possible consequence concerns identity approval, not because
  exploitation is confirmed.

## Review provenance

Run: `20261010-0414-bd8d5d7-722d`. Correctness and security only, nofix. This is collection from a stopped partial run.

Originating reviewers, passes, and local finding identifiers: `ui_desktop_13_cor:p1:F4`.

- `ui_desktop_13_cor:p1:F4`: confidence as filed: possible; suggested bucket as filed: highest.

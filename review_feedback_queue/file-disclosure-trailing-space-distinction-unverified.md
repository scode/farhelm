# Filename distinction in OSC file disclosures remains unresolved

Reviewed commit: afc828776596f2e6639d16d4203ccc8b3d0f5862

## TLDR

Some supported filenames may remain visually ambiguous in the hover that authorizes transfer, especially names ending in
a space. No concrete misleading transfer or harmful outcome was established; this record preserves an unresolved
presentation and coverage question.

## Details

Suggested bucket: provisionally `highest` only for a potential disclosure-dependent authorization consequence.
Confidence: low that the residual is a security defect; the presentation-policy limits are confirmed, but the required
rendering collision and user consequence remain unverified. Provenance: an Astra high auditor investigated an archived
lead, followed by a fresh Astra high reviewer. Neither approved exclusion of the trailing-space residual. Proposed drop:
the narrower character-presentation contract; that basis remained ambiguous under the recording rules.

`crates/farhelm-ui/assets/terminal-files.js:81–97` accepts percent-decoded spaces in OSC file targets. The name and
directory occupy separate lines (`:303–310`). `crates/farhelm-ui/assets/terminal-links.js:342–363` escapes listed unsafe
characters while leaving ordinary spaces and ordinary right-to-left letters intact. `app.css:1253–1256` preserves spaces
without marking the filename's endpoint; file lines use direction isolation at `:4558–4560`.

A concrete hypothesis to investigate is equal-size `report.txt` and `report.txt%20` targets, with another tooltip line
determining its width. The trailing blank might leave no visible distinction. Neither reviewer rendered that pair,
verified all visual cues, or demonstrated a misleading transfer. A trailing space changes the final extension, so native
opening cannot be assumed. No concrete Unicode-space or ordinary-RTL collision plus harmful transfer was established.

Possible cover: SPEC.md:1680–1682 requires unsafe-character escaping, distinguishable literal escape notation and
preserved repeated spaces. SPEC_impl.md:1866–1868 explicitly supports spaces through OSC file links; the printed-token
whitespace recognition non-goal does not cover their display. The shared policy intentionally uses a known-bad list and
excludes variation selectors (`crates/farhelm-proto/src/text.rs:38–56`), an explicit decision in `TRIAGE_OUTCOMES.md`,
“display-peer-misses-invisible-chars.md”. The stronger identity-display rule is not a universal
filename-distinguishability promise. Do not infer a Unicode-injective display requirement from it.

The ledger's “file-disclosure-misses-invisible-path-characters.md” specifies the narrower escaping/spacing fixes but
also says “distinct paths never look alike”. That leaves the trailing-space consequence unclear. No exact Planned, BUGS,
queue or filter exclusion was found. Triage should first establish the rendering and intended distinction before asking
for a fix; this is not a confirmed exploit or an established failure of the shared unsafe-character policy.

The historical source is the terminal-link-safety report at the parent of 7a40e09b1bcddf5bf1e48681d9e1d7d50337df58.
Current source was inspected independently; historical visual claims were not treated as proof. No runtime tests,
browser/native rendering or harmful transfer were performed.

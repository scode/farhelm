# The legacy quickstart contradicts the supervisor's lifetime

Reviewed commit: 05ce9d59f38bfcaf5cdc40162c26695c37d7e44b

## TLDR

The parked quickstart gives contradictory advice about whether the supervisor remains running after the Mac app quits.

## Details

`docs/old_readme.md:42–43` says both the embedded helm and supervisor stop when the app exits. The same commit updates
the paragraph at `67–69` to say that the supervisor remains running. The implementation agrees with the latter:
`crates/farhelm-ui/src/desktop.rs:1182–1256` detaches it, while `770–782` stops only the helm.

Update the earlier quickstart sentence to distinguish these lifetimes. The file's first line calls it parked, which
limits its audience and may affect whether fixing it is worthwhile. No explicit archival rule accepting this
contradiction was found; the reviewed commit deliberately updates lifecycle guidance elsewhere in this same file.

Suggested bucket: `other`. Confidence: confirmed documentation contradiction, without an established harmful reliance or
product defect. Possible cover: the parked-document context above. Required coverage searches found no matching record
or decision, and no filter clearly covered it.

Provenance: g014/f1; Codex gpt-6.1-sol, high reasoning. Source and documentation were inspected; no native application
experiment ran.

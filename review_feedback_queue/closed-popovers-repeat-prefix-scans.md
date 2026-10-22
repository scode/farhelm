# Closed popovers still incur repeated list-prefix work

Reviewed commit: 616d79f45d6a1d817833291429d144a5c686bb4b

## TLDR

Ordinary list refreshes repeat scans and sorted-prefix construction for displayed rows even when no popup is open. This
adds UI-thread work; noticeable latency at the supported list size has not been measured. No ordinary-use availability
failure is established.

## Details

The introduction at `66f99be64257b183d4cb09acd7c66620732ff5d5`, `crates/farhelm-ui/src/rows.rs:208–252`, repeats
searches and prefix scans per row; `list/view.rs:1344–1374` calls it unconditionally. At the reviewed tip,
`rows.rs:361–369` constructs and sorts prefix collections and `379–396` calls it twice per surviving row, alongside the
older algorithm at `250–261`. `list/view.rs:1463–1518` performs the work before dispatch without checking for an open
slot. Host publication also compares content without that check (`1757–1809`); the error-map companion is at
`view.rs:1247–1269` and `rows.rs:424–429`. The added sorting came through `80a437cc442020e0f8e898adf688332f5ba7c6e6`.
These are one repeated-processing concern, not separate findings per caller.

Suggested bucket: **other** for the demonstrated source cost. Confidence: **confirmed repeated work; practical latency
unmeasured**. `SPEC.md:1512–1520` permits whole-list processing at tens of sessions with a few-hundred-row cap; whether
this extra work merits change needs evidence. Avoid allocating per-anchor prefixes when there is no corresponding open
anchor, or compare a shared projection, if measurements justify it.

Possible cover: the malicious-host native helm/desktop availability extension is explicitly accepted by
`SPEC.md:3397–3403`, `SPEC_impl.md:73–87` and `TRIAGE_OUTCOMES.md:11425–11450`; fresh independent review agreed only in
that scope. Browser-only availability is not explicitly named there, so its coverage remains ambiguous. No material
browser stall, corruption, secret access or other-host loss was established. Preserve that distinction rather than
claiming a confirmed security defect or blanket availability acceptance. No exact coverage was found for ordinary
repeated work. Proposed drop: none for this narrow record.

Provenance: trailing review, private run `4fae501c-3686-4822-aa6c-037266637632`, `g051/f1`, `g053/f1`, independent
`a015/f2`. No timing experiment or runtime reproduction.

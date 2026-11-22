# Destination and mismatch label can be abbreviated, with no pre-open access to the full URL

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

The terminal link panel can abbreviate the destination it will open, with no way to read the complete URL before opening
it.

## Details

**high.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d SPEC.md:3483–3504; SPEC_impl.md:3412–3422;
crates/gui/farhelm-grid-model/src/link_disclosure.rs:27–43, 134–170; crates/gui/farhelm-grid-model/src/links.rs:66–87;
crates/gui/farhelm-gui/src/terminal/disclosure.rs:186–212, 236–268;
crates/gui/farhelm-gui/src/terminal/attached.rs:2578–2583, 2765–2799, 3169–3225, 4247–4262;
crates/gui/farhelm-gui/src/host_text.rs:189–193

Farhelm can open an OSC hyperlink even when its destination panel cannot show the whole URL beforehand. The separate
emphasized parsed host, nondefault port and IPv6 brackets remain whole. The serialized HTTP(S) destination and, when
present, the address-shaped program label under Text says are independently shortened over 300 UTF-16 units to the first
and last 140 plus an omission count. The cut operates on the whole serialized address, so path, query, fragment or other
URL text can fall in the missing middle. The native view receives only the abbreviated strings, full host and optional
tunnel hint. Its text element only draws that presentation. There is no expansion, full-value accessibility label or
copy-target action in the disclosure path. Terminal Copy uses selected cell text, which need not contain the OSC target.
Clicking supplies the full parsed destination to the ordinary browser opener, which does not provide a pre-open
disclosure. Current wording explicitly promises the exact target in the hover and calls that hover the whole safeguard,
with no long-target exception or alternative affordance. The behavior pre-existed the patch range. The reviewer
suggested high for materially incomplete destination disclosure; a highest claim still requires a concrete
security-impacting route. Confidence: confirmed. Possible coverage: queue coverage unknown.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `contract-corrections/CC-06`.

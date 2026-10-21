# Removal destinations collapse distinguishing spaces

Reviewed commit: fe0348ef8d4fabc3c6140a4fd89fb19dd34178b1

## TLDR

Distinct registered destinations differing only in repeated ordinary spaces can look identical in removal and refusal
dialogs. Scrolling cannot recover the distinction. Live SSH reachability and broader harmful reliance are unverified.

## Details

`crates/farhelm-ui/src/hosts/settings_dialog.rs:854,882–884` and `hosts.rs:139,174–175` render `display_peer`
destinations in ordinary paragraphs. The renderer preserves normal spaces (`peer.rs:63–85`); paragraph CSS wraps without
preserving whitespace (`assets/app.css:3321–3324`), and `.peer-value` only isolates bidi (`4802–4804`). Thus `node one`
and `node  one` collapse to the same visible text.

The registry accepts raw strings apart from empty/leading-hyphen/NUL checks (`farhelm-helm/src/store.rs:1685–1687`),
preserves them on registration (`5702–5755`) and distinguishes them in its index (`2171–2172`). Alias checking does not
merge the unaliased destinations (`1565–1628`). Preserve distinguishing whitespace in these body fields. Setup/uninstall
plans already use pre-wrap and are outside this mechanism; shortening their headings is not itself the defect.

Suggested bucket: **other** for the demonstrated disambiguation loss. Confidence: **confirmed static
accepted-input/render mismatch; browser execution and valid live SSH examples unverified**. Remove leaves remote
supervisor/sessions running (`hosts.rs:194–201`), and no wrong-machine uninstall, secret loss or user-work loss was
established. Fresh independent `audit-discovery-display-dataflows` initially reserved the highest boundary, then agreed
to exclude that stronger consequence after tracing the callbacks. Removal uses the captured HostId and forgets the
registry entry; remote work survives. Refusal offers no setup/uninstall dispatch through its collapsed paragraph, while
those actions separately show whitespace-preserving plans. A new concrete harmful chain would need reassessment; the
audit does not prove every SSH destination or future action safe.

Possible cover: `SPEC.md:1382–1384` requires body destinations. File disclosure and installation-ID whitespace outcomes
(`TRIAGE_OUTCOMES.md:13689–13721`) concern other values/renderers. No exact Planned, BUGS or queue match was found; the
scrolling filter does not restore permanently collapsed distinctions. Proposed drop: none for the narrow finding.
Provenance: `audit-host-destination-disclosure/f3`, discovery pass 1, trailing review through the reviewed commit. The
old heading-omission exclusions remain narrow. No runtime reproduction was performed.

Original finding IDs: `audit-host-destination-disclosure/f3`, `audit-discovery-display-dataflows/f1`.

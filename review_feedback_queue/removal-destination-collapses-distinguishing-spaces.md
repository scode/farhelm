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

### Manual supervisor-start guidance has the same presentation defect

Additional reviewed commit: `6788c67974e6ae533ffd3fb58295a3ec3f171d17`. The SSH exit-75 startup remedy preserves the
actual quoted executable/state command (`crates/farhelm-helm/src/manager.rs:4812–4826,4866–4868,4975`), but the hosts
detail renders it through normal-whitespace inline peer spans (`crates/farhelm-ui/src/hosts.rs:711–715,4271–4272`;
`peer.rs:159–179,304–328`; `assets/app.css:4760–4765,4781–4787,4863–4865,5196–5202`). Registered paths retain repeated
ordinary spaces (`farhelm-helm/src/store.rs:1737–1768,5770–5773`). The displayed command can therefore collapse a
distinction present in the exact Rust command. This predates the reviewed patch.

Suggested bucket: **other** for confirmed visual mismatch; possible highest impact remains conditional. Unlike Remove,
this guidance is a manual shell action, so the earlier captured-HostId refutation does not exclude a harmful copied
command. However, browser clipboard serialization and a supported wrong-installation or work-loss chain remain missing
premises. No pinned source establishes them, and no reproduction occurred. The earlier Debug-escaping repair and
descriptive fallback address a different transformation; they do not restore whitespace here.

Possible cover: this is additional substance for the same renderer concern, not an already-covered identical trigger. No
exact accepted exclusion was established. Proposed drop: none. Provenance: trailing review through
`616d79f45d6a1d817833291429d144a5c686bb4b`, run `4fae501c-3686-4822-aa6c-037266637632`, `g073/f1`. That investigation
stops incomplete on copying and harmful-action premises; the displayed-space mismatch itself is confirmed.

# A highlight change can arm Replace while the screen still offers to apply a template

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Moving the highlight to a recent setup can arm Replace before the new Enter action appears, allowing deletion under the
earlier template cue.

## Details

**highest.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d:crates/gui/farhelm-gui/src/launcher/view/search/start.rs:69–115,138–145,267–305;
crates/gui/farhelm-gui/src/launcher/view/search.rs:245–260;
crates/gui/farhelm-gui/src/launcher/view.rs:1250–1256,1297–1316,2143–2155;
crates/gui/farhelm-gui/src/prompt.rs:1193–1215; vendor/gpui-pre/src/window.rs:3887–3901,6503–6509

Enter can delete the source while the last presented highlight is on a template that would only fill the form. LS-06 is
the same route: in a wide pane, a navigation key moves from a template to the preceding recent setup, changing the live
Enter action ahead of presentation. The unchanged SourceConsent and unit offer let ActionFrame reuse its previous shown
value. That value has only process and tab consent, with no command-field action/row identity. A valid ordinary recent
reaches shared Replace and can remove the source after creation
(crates/farhelm-helm/src/sessions.rs:4870–4909,5022–5028). SPEC.md:2194–2200,5572–5581 and SPEC_impl.md:984–988 at the
reviewed hash require shown action admission. Treat LS-06 as duplicate evidence for this ID. Resize and scrolling use
different triggers but can share a fix. The reviewer suggested highest for the destructive verb change; confidence
source-confirmed under documented toolkit scheduling, timing frequency unmeasured; ; Possible coverage: LS-06.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as
`launcher-match-actions/recent-enter-drawn-binding`, with `launcher-search/LS-06`, `launcher-matches/LM-09`.

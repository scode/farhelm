# Replace can submit before the checkout warning appears

Reviewed commit: 1a53d9d35889d7dde96e31320cc1681dd013737d

## TLDR

Replace with can submit a fresh checkout before its updated warning is shown, allowing the previous checkout to move to
trash without the displayed disclosure.

## Details

**highest.** Source inspection.

1a53d9d35889d7dde96e31320cc1681dd013737d
crates/gui/farhelm-gui/src/launcher/view.rs:568-573,961-974,1297-1316,1322-1356,2127-2155,2439,
view/checkout.rs:445-464,471-497,550-590,721-764,792-815,1024-1049, request.rs:389-410,598-625,
crates/gui/farhelm-gui/src/operations.rs:620-645, prompt.rs:1176-1215,
vendor/gpui-pre/src/window.rs:2043-2079,3887-3901,6503-6509; SPEC.md:2159-2168,5572-5576 and SPEC_impl.md:984-994 at
pinned tip

The first recent-setup press waits for a matching fresh checkout preview. Preview readiness and presentation of the
changed checkout warning are separate. On Wayland, an outstanding compositor callback can defer presentation while
keyboard dispatch and an asynchronous preview reply still run. GPUI can draw the updated warning before dispatching a
key without making that frame visible. A subsequent ordinary press can consume Ready together with the existing shown
source-liveness consent. Those checks allow the request to use a fresh clone and remove its source, while the checkout
warning was absent from the last visible frame. See `launcher/view/checkout.rs:733–764,792–815`,
`launcher/view.rs:1297–1316,1322–1356`, `prompt.rs:1193–1215`,
`vendor/gpui-pre-linux/src/linux/wayland/window.rs:1012–1018,1043–1056,2065–2073` and `SPEC.md:2159–2168,5572–5577` at
the pinned review tip. Reuse the shown-action mechanism to bind the admitted checkout transition. Incidence was not
measured.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-matches/LM-10`.

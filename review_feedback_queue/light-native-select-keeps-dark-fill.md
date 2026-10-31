# Light dropdown text can keep a dark native fill

Reviewed commit: d6645f771bee8ed03a66486ef8755834937d759d

## TLDR

On the inspected Linux WebKit/Adwaita path, Light template dropdowns combine dark text with a dark native background.

## Details

Suggested bucket: **other**. Confidence: **verified**. Source-confirmed assessment; conditional routes retain their
prerequisites and no runtime observation is claimed.

The review traced the actual Playwright 1.64 WebKit revision and primary native paint order, resolving the earlier
missing engine premise. Source establishes the style/paint conflict for that path. It does not establish Safari behavior
or observed native pixels; no browser was launched.

Source locations: `crates/farhelm-ui/assets/app.css:3583-3608 at d6645f771bee8ed03a66486ef8755834937d759d`. Historical
revision qualifiers in those references are retained.

Proposed drop: none for the retained scope. Possible coverage and its limits are preserved in the assessment; an
ambiguous match is not acceptance.

## Recorded source assessment

Follow-up resolves the named engine premise through matching PRIMARY source; previous uncertainty events remain
unchanged. Reviewed commit:d6645f771bee8ed03a66486ef8755834937d759d. TLDR: template dropdowns in Light appearance have
dark text on a dark surface on Linux WebKit's Adwaita path. Farhelm CSS:367-386 supplies #1b2130 text in Light,
while3583-3608 gives native template selects color-scheme:dark. Actual Agent type producer and nine-option native-select
route were established previously. Repo lock pins Playwright1.64.0. Official microsoft/playwright v1.64.0
browsers.json:25-33 pins WebKit2370/27.2; UPSTREAM_CONFIG.sh:1-3 pins upstream56453fdfe0b0ca6258c23e6453b34f70b885d13f.
Official pw_run.sh:24-36 selects minibrowser-wpe for Linux headless and GTK otherwise. Matching WebKit
OptionsWPE.cmake:437-441 enables THEME_ADWAITA. At that upstream revision, Document.cpp:10032-10046 resolves an element
dark-only scheme to dark independently of page choice. RenderObject.cpp:2017-2019 and RenderTheme.cpp:785-786 pass
DarkAppearance to native controls. RenderThemeAdwaita.cpp:79-119 supplies Menulist and styled MenulistButton-decoration
controls. RenderBox.cpp:1857-1879 paints the styled decoration AFTER the explicit CSS background.
ControlFactoryAdwaita.cpp:96-103 maps BOTH paths to MenuListAdwaita; MenuListAdwaita.cpp:44-67 calls
ButtonControlAdwaita::drawButton. That draw routine:48-93 fills the full native button from dark theme constants,
independent of the author background. Adwaita.h:96-99 sets ordinary RGB52,52,52, hovered60,60,60, pressed30,30,30. This
resolves why the explicit light --well CSS background does not prevent the dark surface. MenuListAdwaita paints the
surface/arrow, not replacement label ink; Farhelm's explicit #1b2130 author ink remains. RenderTheme menu style
adjuster:1535-1539 changes only overflow, not text color. The bootstrap.diff source was searched for theme/control
changes and contains no matching modifications to these painting files; no runtime/binary inspection is claimed.
Confidence:confirmed source route for supported Linux WPE/Adwaita, no runtime reproduction, no general claim about
Cocoa/Safari or other native themes. Suggested bucket:other. proposed_drop:none. possible_cover:none matching under
previous exact coverage investigation. Historical report images remain unauthenticated and are not proof; introduction
by host-short-names is not claimed. Source URLs:
https://github.com/microsoft/playwright/blob/v1.64.0/browser_patches/webkit/pw_run.sh ;
https://github.com/microsoft/playwright/blob/v1.64.0/browser_patches/webkit/UPSTREAM_CONFIG.sh ;
https://github.com/WebKit/WebKit/blob/56453fdfe0b0ca6258c23e6453b34f70b885d13f/Source/WebCore/platform/graphics/adwaita/MenuListAdwaita.cpp
;
https://github.com/WebKit/WebKit/blob/56453fdfe0b0ca6258c23e6453b34f70b885d13f/Source/WebCore/platform/graphics/adwaita/ButtonControlAdwaita.cpp
; https://github.com/WebKit/WebKit/blob/56453fdfe0b0ca6258c23e6453b34f70b885d13f/Source/WebCore/rendering/RenderBox.cpp
. No security or user-work-loss consequence alleged.

Provenance: trailing-review run `1e4abedb-1946-4bfb-93a4-fd2f4e241acd`; `g105/f3`. These are original reviewer
identities for this concern, not additional bugs. Full revisions, withdrawals and independent verdicts remain in the
private review evidence. No fix or triage decision was executed.

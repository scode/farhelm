# Very short launchers may leave notices unreachable

Reviewed commit: da9d7411fd5c69255ea6d0b857a6a3aa48bd0c6c

## TLDR

In a sufficiently short window, the launcher's footer can run out of space for explanations and YOLO answers. A fixed
CSS sizing shortfall is established, but a supported window and input sequence demonstrating inaccessible content has
not been established.

## Details

Suggested bucket: **other**. Confidence: **unverified after investigation** as a user-facing reachability failure. No
ordinary-size failure, unauthorized approval or loss of work was demonstrated. A harmful confirmation remaining usable
while its required disclosure cannot be reached would justify reassessing the bucket; that route remains missing.

The launcher change is `09b806f7f08f436afacf0ac1f7cb867c8a7ef126`. At the reviewed tip,
`crates/farhelm-ui/assets/app.css:5919–5926` caps the form and hides overflow; `6019–6025` preserves a 48px body
minimum. The footer at `6971–6987` is capped at 55% of the available height and has a nonshrinking action row. Notices
at `7013–7023` may shrink to zero. The compact rules at `7036–7060` reduce chrome and wrap actions, but supply no
positive notice-height floor. Destructive warnings and the inline question are inside this region in
`crates/farhelm-ui/src/list/create_form.rs:7560–7735`.

At 390×100 CSS pixels, the 12px vertical margins leave a 41.8px footer cap. Compact vertical padding and border take
17px, the child gap takes 6px, and even one ordinary action row needs 28px. That leaves −9.2px before summary text or
additional rows. This conservative bound exhausts the notice budget at heights of at most 116.72px. It does not
establish collapse at 480, 481, 500, 600, 601 or 844px: the responsive rules and source-level test assertions address
those larger sizes. Fonts and wrapping can change other thresholds.

The investigation followed the shared scroll-region keyboard entry and focus handling
(`crates/farhelm-ui/src/modal_dialog.rs:463–610`), YOLO Cancel scrolling (`src/yolo_confirm.rs:194–216,261–282`), window
construction and restoration, and the capture and regression sources. Focus handling does not enlarge the notice region.
Version-matched Dioxus Desktop 0.7.10 and Tao 0.34.8 source sets no default minimum size; the inspected macOS and Linux
paths apply a minimum only when supplied. That resolves the framework-default question without establishing
platform-reserved limits, actual WebView viewport delivery, computed clipping, focus-induced recovery or pointer access
for the complete failing case. No browser or native reproduction ran, and inspected test assertions are not passing-test
evidence. A supported viewport and populated draft, measured rectangles, and keyboard/pointer observations would settle
those premises. A scrolling fallback for the whole card is one possible remedy if confirmed.

Possible cover: `SPEC.md:907–917` permits bounded scrolling and visually truncated summaries; `SPEC_impl.md:522–531`
still requires short-window reachability. `FILTER.md`, "Dialog explanations out of view but reachable by scrolling", and
the two decisions at `TRIAGE_OUTCOMES.md:13592–13622` accept reachable explanations only. The existing Host Settings and
host-rail records concern different surfaces. No exact Planned, unlanded-plan, BUGS or queue coverage was found.
Proposed drop: none; independent auditors could not justify excluding unreachable content under reachable-scrolling
acceptance.

Provenance: trailing-review run `2b7a8eaf-c4e9-42ab-be93-1acf29106132`, principally `g06/f2`, with `g05/f4`, `g09/f2`,
`g10/f2`, `g13/f5`, `g14/f1`, `a02/f5` and `a03/f1` retaining the same conditional mechanism. These are duplicate
observations, not separate failures.

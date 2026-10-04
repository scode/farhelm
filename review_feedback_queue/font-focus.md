# Text-size buttons leave focus outside the terminal at the limits

Reviewed commit: 6943f94c24a193bad9433e718bbe4e1fb6b237dc

## TLDR

Text-size buttons leave focus outside the terminal at the limits.

## Details

`F28 / COR-FONT-FOCUS` — **definite** — `crates/farhelm-ui/assets/terminal.js:2755` — Text-size buttons leave focus
outside the terminal at the limits

At the maximum text size of 28, another A+ click leaves keyboard focus on the button; the same happens with A− at the
minimum size of 9. These buttons remain enabled at the limits. The text-size function clamps the requested size and
returns immediately when it would not change, before the code that returns focus to the terminal.

After that ordinary click, typing no longer reaches the terminal, and Enter activates the same no-op button again.
Return focus even when the size is unchanged, while keeping font updates and terminal resizing conditional on an actual
size change. Extend the button-click coverage to both limits.

Suggested bucket: high

Possible cover: none

Caveats: No browser reproduction was performed. Clicking the terminal restores input. The keyboard shortcuts do not have
this button-focus failure.

## Review context

Review: seven-day main diff `7cc0681495159e4a6abe543dffdff1acc0720675..6943f94c24a193bad9433e718bbe4e1fb6b237dc`,
correctness and security only, nofix.

Confidence as retained: **definite**. Suggested queue bucket: **high**.

Originating reviewers and passes: `ui_edges p1`.

Possible cover recorded during collection: none identified.

Collection caveats: No browser reproduction; clicking terminal recovers; shortcuts unaffected.

## Filed reviewer metadata

- `ui_edges p1`: confidence as filed: **definite / confirmed** by the enabled button and early-return path; no browser
  reproduction ran. Suggested bucket as filed: **high**.

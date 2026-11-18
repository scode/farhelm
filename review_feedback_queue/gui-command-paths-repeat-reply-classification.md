# GUI command paths classify replies separately

Reviewed commit: c9575db61abb31bebe43da0f11a76712e18725c1

## TLDR

Two GUI request paths repeat the classification of the same helm replies and refusal messages.

## Details

Haiku pass, checked by gpt-6.2-sol. Confidence is source-confirmed for the cited locations. The suggested changes were
evaluated by source inspection.

Both paths classify the same reply statuses and extract the same refusal words. Have the keyless path project
`Outcome::from_reply` to body or words, retaining its separate handling for command errors.

Reviewed locations: `crates/gui/farhelm-gui/src/operations.rs:546-556` and :633-643. Haiku out-P5.md §1. Suggested
bucket **other**.

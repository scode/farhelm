# Loading the model catalog can clear Enter’s selection even though the chosen model is still listed

Reviewed commit: 14fda80233aa69231c0b44431fc950764fbcdfd1

## TLDR

Loading the model catalog can clear Enter's selection while the same model is still listed.

## Details

**other.** Source inspection.

14fda80233aa69231c0b44431fc950764fbcdfd1 crates/gui/farhelm-gui/src/launcher/view/search.rs:194–202, 263–267, 780–797;
1a53d9d35889d7dde96e31320cc1681dd013737d crates/gui/farhelm-gui/src/launcher/search.rs:607–645;
crates/gui/farhelm-gui/src/launcher/view/search.rs:198–206, 224–242, 268–271, 794–811;
crates/gui/farhelm-gui/src/launcher/view.rs:499–508, 1444–1449; crates/gui/farhelm-gui/src/launcher/draft.rs:775–797

After the user highlights a model found in past launches, a catalog response can leave the same model visible but remove
the highlight and make Enter do nothing until the user navigates again. The model catalog promotes a matching current
model and suppresses the equivalent history row. The explicit selection still holds the history row variant, while
same_search_choice compares two catalog model rows by model and harness and falls back to variant equality for the
history-to-catalog change. A catalog response retains the old choice. The draft already treats a recognized history
model as the corresponding catalog edit, so for an identical ID and harness the lost selection is a change in
representation, not removal of the model. SPEC.md at the pinned tip:1098–1105, 1147–1149 describes model search and
visible selection; the source's identity-preserving selection can preserve this one too. The reviewer suggested other,
confidence confirmed for the state transition when the catalog completes after the highlight, runtime timing and
frequency unmeasured. ; Possible coverage: related LS-02 loses the choice on reconnect and falls back to a different
default, while LM-06/LMC-05 concern an actual recent setup disappearance.

Review evidence is retained under run `e551ad05-1866-44a9-b97b-9e47d1d67375` as `launcher-search/LS-11`.

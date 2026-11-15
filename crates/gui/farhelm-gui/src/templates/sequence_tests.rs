//! Tests of the quick-launch sequence model, without a window: how each key changes the queue and the word, what the
//! pill shows, and what Enter resolves a sequence to.
//!
//! The oracle is SPEC.md, "Quick-launch sequences", SPEC_impl.md, "Launch templates", and the old app's
//! `launch_sequence.rs` tests and `js-tests/template-shortcuts.test.js` cases at reference commit `24bc0580`, ported
//! where they test behavior this model owns. Enter launches what this model returns with no further check of the
//! names, so a precedence slip here (a valid prefix of a refused sequence applying, a moved key substituting another
//! template) is a launch the user did not ask for.

use std::collections::BTreeMap;

use super::Hint;
use super::sequence::{
    Binding, CANDIDATE_LIMIT, Chip, Entry, Input, Outcome, Reference, Refusal, Sequence,
    match_word, matching_names, printable,
};
use super::shortcuts::Bindings;
use super::summary::Part;

// ---------------------------------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------------------------------

/// Bindings with `marked` holding ⚡ and `keys` as digit-to-name pairs.
fn bindings(marked: Option<&str>, keys: &[(&str, &str)]) -> Bindings {
    Bindings {
        marked: marked.map(str::to_owned),
        keys: keys
            .iter()
            .map(|(key, name)| ((*key).to_owned(), (*name).to_owned()))
            .collect::<BTreeMap<_, _>>(),
    }
}

/// An armed sequence with no bindings, for tests about the queue alone.
fn armed() -> Sequence {
    Sequence::new(Bindings::default())
}

/// The store has no template names to offer yet (Tab and the candidates take `None` then).
const NO_NAMES: Option<&[&str]> = None;

/// Type `text` one character at a time, as the GPUI layer would deliver it.
fn type_text(sequence: &mut Sequence, text: &str) {
    for c in text.chars() {
        sequence.input(Input::Character(c));
    }
}

fn digit(key: &str) -> Input {
    Input::Digit {
        key: key.to_owned(),
        text: key.chars().next().unwrap(),
    }
}

fn chord(key: &str) -> Input {
    Input::NumberChord(key.to_owned())
}

fn word(text: &str) -> Entry {
    Entry::Word(text.to_owned())
}

fn number(key: &str) -> Entry {
    Entry::Number(key.to_owned())
}

/// The names an outcome applies, or a panic naming the outcome it was instead.
fn applied(outcome: Outcome) -> Vec<String> {
    match outcome {
        Outcome::Apply(names) => names,
        other => panic!("expected Apply, got {other:?}"),
    }
}

/// A sentence's parts joined as plain text, for comparing wording.
fn plain(parts: &[Part]) -> String {
    parts
        .iter()
        .map(|part| match part {
            Part::Words(text) | Part::Host(text) | Part::Mono(text) => text.as_str(),
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Input: the queue and the unfinished word
// ---------------------------------------------------------------------------------------------------------------------

/// The queue is the whole sequence's meaning, so its order must be the order the user typed. A bare digit extends a
/// word in progress but queues its number key with no word; a chord queues the unfinished word before its own
/// reference; Backspace right after a chord has no word to edit and must not remove the queued entry; Space queues the
/// word alone; and the same word twice is two entries.
#[test]
fn keys_queue_entries_in_typing_order() {
    let mut sequence = armed();
    sequence.input(digit("1"));
    type_text(&mut sequence, "Codex");
    sequence.input(digit("2"));
    sequence.input(chord("3"));
    type_text(&mut sequence, "project");
    sequence.input(Input::MarkChord);
    sequence.input(Input::Backspace);
    sequence.input(Input::Space);
    type_text(&mut sequence, "project");
    sequence.input(Input::Space);
    assert_eq!(
        sequence.entries(),
        [
            number("1"),
            word("Codex2"),
            number("3"),
            word("project"),
            Entry::Mark,
            word("project"),
        ]
    );
    assert_eq!(sequence.word(), "");
}

/// A bare digit extends a word with the character the key prints, not its digit: on a layout where the physical 2 key
/// prints "é" the word gets "é", while the queued number key stays the physical digit (SPEC.md: matching "uses the
/// physical digit key, independent of the printed character").
#[test]
fn a_bare_digit_extends_a_word_with_its_printed_character() {
    let mut sequence = armed();
    sequence.input(Input::Digit {
        key: "2".into(),
        text: 'é',
    });
    type_text(&mut sequence, "caf");
    sequence.input(Input::Digit {
        key: "2".into(),
        text: 'é',
    });
    assert_eq!(sequence.entries(), [number("2")]);
    assert_eq!(sequence.word(), "café");
}

/// Space with no word in progress queues nothing (SPEC.md: "an empty Space does nothing"), so stray separators cannot
/// add empty words that Enter would then refuse.
#[test]
fn an_empty_space_queues_nothing() {
    let mut sequence = armed();
    sequence.input(Input::Space);
    type_text(&mut sequence, "a");
    sequence.input(Input::Space);
    sequence.input(Input::Space);
    assert_eq!(sequence.entries(), [word("a")]);
}

/// Backspace removes one Unicode scalar, so an astral character such as an emoji goes in one press rather than leaving
/// half a surrogate pair, and once the word is empty further presses leave the queued entries alone (SPEC.md:
/// "Backspace removes the last character of the unfinished word, never a queued entry").
#[test]
fn backspace_edits_only_the_unfinished_word_by_scalar() {
    let mut sequence = armed();
    type_text(&mut sequence, "base");
    sequence.input(Input::Space);
    type_text(&mut sequence, "a🦀");
    sequence.input(Input::Backspace);
    assert_eq!(sequence.word(), "a");
    sequence.input(Input::Backspace);
    sequence.input(Input::Backspace);
    assert_eq!(sequence.entries(), [word("base")]);
    assert_eq!(sequence.word(), "");
}

/// The armed listener's text rule: exactly one Unicode scalar that is not a control character types into the word, so
/// Shift capitals, shifted digits, accented letters and emoji count, while a dead key's name, a C0 or C1 control
/// character and empty text do not (the GPUI layer cancels the sequence for those).
#[test]
fn printable_admits_one_non_control_scalar() {
    for text in ["a", "A", "!", "é", "🦀", " "] {
        assert_eq!(printable(text), text.chars().next(), "{text:?}");
    }
    for text in ["", "Dead", "ab", "\u{1}", "\u{7f}", "\u{85}"] {
        assert_eq!(printable(text), None, "{text:?}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Tab
// ---------------------------------------------------------------------------------------------------------------------

/// Tab is advisory: with the names unknown it does nothing (and nothing remembers the press to replay later). Once
/// names are known it queues the full name as one entry, spaces included (SPEC.md: "Tab can queue an existing name
/// containing spaces as one entry"), and an empty word completes nothing even when only one template exists.
#[test]
fn tab_needs_known_names_and_a_word() {
    let mut sequence = armed();
    type_text(&mut sequence, "old");
    sequence.tab(NO_NAMES);
    assert_eq!(sequence.word(), "old");
    assert!(sequence.entries().is_empty());
    let names = ["old project"];
    sequence.tab(Some(&names));
    assert_eq!(sequence.entries(), [word("old project")]);
    assert_eq!(sequence.word(), "");
    sequence.tab(Some(&names));
    assert_eq!(
        sequence.entries().len(),
        1,
        "an empty word must not complete the sole name"
    );
}

/// Tab completes only a word that names one template, by Enter's precedence: an ambiguous prefix and an unknown word
/// stay as typed, and an exact name wins over longer names it is a prefix of (SPEC.md: "an exact match wins over
/// longer prefixes").
#[test]
fn tab_completes_only_a_unique_match() {
    let names = ["codex", "codex-yolo", "claude"];
    let mut sequence = armed();
    type_text(&mut sequence, "co");
    sequence.tab(Some(&names));
    assert_eq!(
        sequence.word(),
        "co",
        "ambiguous between codex and codex-yolo"
    );
    type_text(&mut sequence, "dex");
    sequence.tab(Some(&names));
    assert_eq!(sequence.entries(), [word("codex")]);
    type_text(&mut sequence, "zzz");
    sequence.tab(Some(&names));
    assert_eq!(sequence.word(), "zzz");
    assert_eq!(sequence.entries().len(), 1);
}

/// The names Tab saw never decide what Enter applies (SPEC.md: the catalog "never gates Enter or decides what Enter
/// launches"), and SPEC_impl.md says Tab's completed spelling still goes through the matching rules at Enter. So a
/// completed name whose template is gone by Enter is refused as an unknown word rather than applying what Tab saw, and
/// one that is now only a prefix of a single template resolves to that template.
#[test]
fn a_completed_name_is_matched_again_at_enter() {
    let completed = || {
        let mut sequence = armed();
        type_text(&mut sequence, "old");
        sequence.tab(Some(&["old project"]));
        assert_eq!(
            sequence.entries(),
            [word("old project")],
            "premise: Tab completed the name"
        );
        sequence
    };
    assert_eq!(
        completed().enter(&["replacement"]),
        Outcome::Refused(Refusal::UnknownWord("old project".into()))
    );
    assert_eq!(
        applied(completed().enter(&["old project 2"])),
        ["old project 2"]
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Matching and candidates
// ---------------------------------------------------------------------------------------------------------------------

/// SPEC.md's precedence: the case-sensitive exact name wins even over case competitors and longer prefixes; then a
/// unique lowercase-exact name; then a unique lowercase prefix. A tie in the lowercase-exact tier is ambiguous between
/// exactly that tier's names, and longer prefix matches are not competitors. The lowercase is Rust's Unicode lowercase,
/// not full case folding: "İ" lowercases to "i" plus a combining dot, and "ß" does not match "SS".
#[test]
fn words_match_exact_then_lowercase_exact_then_prefix() {
    let names = ["codex-yolo", "Codex", "codex", "İstanbul", "Straße"];
    assert_eq!(match_word("Codex", &names), Ok("Codex"));
    assert_eq!(match_word("codex", &names), Ok("codex"));
    assert_eq!(
        match_word("CODEX", &names),
        Err(Refusal::AmbiguousWord {
            word: "CODEX".into(),
            candidates: vec!["Codex".into(), "codex".into()],
        }),
        "the lowercase-exact tier caused this ambiguity, so codex-yolo is not a competitor"
    );
    assert_eq!(match_word("CODEX-Y", &names), Ok("codex-yolo"));
    assert_eq!(match_word("i\u{307}s", &names), Ok("İstanbul"));
    assert_eq!(
        match_word("istan", &names),
        Err(Refusal::UnknownWord("istan".into()))
    );
    assert_eq!(
        match_word("STRASSE", &names),
        Err(Refusal::UnknownWord("STRASSE".into()))
    );
    assert_eq!(match_word("CODEX", &["codex-yolo", "Codex"]), Ok("Codex"));
}

/// The candidate order is exact first, then lowercase exact, then prefixes, catalog order within each tier; a name
/// listed twice is one name, and non-matches and an empty word give nothing. This is the order the pill's list shows.
#[test]
fn matching_names_are_ordered_by_tier_then_catalog() {
    let names = [
        "co-five",
        "elsewhere",
        "CO",
        "co-one",
        "co",
        "co-two",
        "co-three",
        "co-four",
        "co",
    ];
    assert_eq!(
        matching_names("co", &names),
        [
            "co", "CO", "co-five", "co-one", "co-two", "co-three", "co-four"
        ]
    );
    assert!(matching_names("missing", &names).is_empty());
    assert!(matching_names("", &names).is_empty());
    assert_eq!(match_word("co-one", &["co-one", "co-one"]), Ok("co-one"));
}

/// The list under the pill shows at most five names, but a sixth match must still count: a display cap that decided
/// ambiguity would silently make a broad prefix look unique. Tab and Enter both see all six.
#[test]
fn the_candidate_cap_never_limits_ambiguity() {
    let names = [
        "co-one", "co-two", "co-three", "co-four", "co-five", "co-six",
    ];
    let mut sequence = armed();
    type_text(&mut sequence, "co");
    assert_eq!(CANDIDATE_LIMIT, 5);
    assert_eq!(sequence.view(Some(&names)).candidates, names[..5]);
    sequence.tab(Some(&names));
    assert_eq!(
        sequence.word(),
        "co",
        "Tab must not complete an ambiguous word"
    );
    match sequence.enter(&names) {
        Outcome::Refused(Refusal::AmbiguousWord { candidates, .. }) => {
            assert_eq!(
                candidates, names,
                "the sixth match belongs in the ambiguity"
            );
        }
        other => panic!("expected an ambiguity, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// The pill
// ---------------------------------------------------------------------------------------------------------------------

/// The pill shows each queued entry through the sequence's own copy of the bindings: a bound number key or ⚡ as its
/// template's name, an unbound one in the old app's words, a typed word or completed name as itself; then the word in
/// progress and its candidates among the names passed in. Names stay raw, so the drawer's escaping cannot change the
/// spelling Enter looks up.
#[test]
fn the_view_shows_chips_the_word_and_candidates() {
    let names = ["review", "release", "other"];
    let mut sequence = Sequence::new(bindings(Some("bolt\u{202e}"), &[("1", "one\n")]));
    sequence.input(digit("1"));
    sequence.input(digit("2"));
    sequence.input(Input::MarkChord);
    type_text(&mut sequence, "x");
    sequence.input(Input::Space);
    type_text(&mut sequence, "rev");
    sequence.tab(Some(&names));
    type_text(&mut sequence, "re");
    let view = sequence.view(Some(&names));
    assert_eq!(
        view.chips,
        [
            Chip::Name("one\n"),
            Chip::Unbound("Unbound key 2".into()),
            Chip::Name("bolt\u{202e}"),
            Chip::Name("x"),
            Chip::Name("review"),
        ]
    );
    assert_eq!(view.word, "re");
    assert_eq!(view.candidates, ["review", "release"]);
    assert!(
        sequence.view(NO_NAMES).candidates.is_empty(),
        "no names yet, no candidates"
    );

    let unmarked = {
        let mut sequence = armed();
        sequence.input(Input::MarkChord);
        sequence
    };
    assert_eq!(
        unmarked.view(Some(&names)).chips,
        [Chip::Unbound("Unbound ⚡".into())]
    );
    assert!(
        unmarked.view(Some(&names)).candidates.is_empty(),
        "no word, no candidates"
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Enter
// ---------------------------------------------------------------------------------------------------------------------

/// Enter with nothing queued cancels without opening anything (SPEC.md), and Enter queues an unfinished word first, so
/// typing a name and pressing Enter needs no Space.
#[test]
fn enter_queues_the_word_and_an_empty_sequence_is_nothing() {
    assert_eq!(armed().enter(&["project"]), Outcome::Nothing);
    let mut sequence = armed();
    type_text(&mut sequence, "proj");
    assert_eq!(applied(sequence.enter(&["project"])), ["project"]);
}

/// A sequence applies in queue order with duplicates kept, number keys and ⚡ through the copy the sequence took when
/// it started and typed words through matching. Because the sequence holds that copy and never reads the bindings
/// again, a key moved to another template after the prefix cannot change what is applied.
#[test]
fn enter_applies_in_order_through_the_captured_bindings() {
    let mut sequence = Sequence::new(bindings(Some("second"), &[("1", "first")]));
    sequence.input(digit("1"));
    sequence.input(Input::MarkChord);
    type_text(&mut sequence, "SEC");
    sequence.input(chord("1"));
    assert_eq!(
        applied(sequence.enter(&["first", "second"])),
        ["first", "second", "second", "first"]
    );
}

/// A number key's captured name that no longer exists refuses the whole sequence naming it, even when a template with
/// the same name in another case exists, or one the old name is a prefix of: a renamed or deleted captured binding
/// "cannot select a different template" (SPEC_impl.md), so bindings are looked up by exact name, never by the typed
/// word's matching rules, and a valid prefix of the sequence must not apply on its own.
#[test]
fn a_missing_captured_name_refuses_the_whole_sequence() {
    let mut sequence = Sequence::new(bindings(None, &[("1", "first"), ("2", "gone")]));
    sequence.input(digit("1"));
    sequence.input(chord("2"));
    assert_eq!(
        sequence.enter(&["first", "Gone", "gone 2"]),
        Outcome::Refused(Refusal::Missing("gone".into()))
    );
}

/// A word's refusal wins over a key anywhere in the sequence (SPEC.md), whether the key is unbound or its captured
/// template is gone, for both an unknown and an ambiguous word; the refusal names the word and, for ambiguity, every
/// competitor. Nothing in the sequence applies, including the valid word before it.
#[test]
fn a_word_refusal_wins_over_any_key_refusal() {
    let templates = ["project", "codex", "codex-yolo"];
    let refusal = |bound: &[(&str, &str)], word: &str| {
        let mut sequence = Sequence::new(bindings(None, bound));
        sequence.input(digit("9"));
        type_text(&mut sequence, "project");
        sequence.input(Input::Space);
        type_text(&mut sequence, word);
        match sequence.enter(&templates) {
            Outcome::Refused(refusal) => refusal,
            other => panic!("expected a refusal for {word:?}, got {other:?}"),
        }
    };
    for bound in [&[][..], &[("9", "gone")][..]] {
        assert_eq!(
            refusal(bound, "unknown"),
            Refusal::UnknownWord("unknown".into()),
            "{bound:?}"
        );
        assert_eq!(
            refusal(bound, "co"),
            Refusal::AmbiguousWord {
                word: "co".into(),
                candidates: vec!["codex".into(), "codex-yolo".into()],
            },
            "{bound:?}"
        );
    }
}

/// A number key or ⚡ the pill showed unbound stays unbound at Enter: a binding added elsewhere since the prefix cannot
/// launch something the pill never showed. The refusal names the first unbound reference and whether the sequence was
/// that reference alone (a word beside it makes it not alone), and a missing captured name elsewhere in the sequence
/// wins over it, as the old dispatcher's fallback order had it.
#[test]
fn an_unbound_reference_refuses_naming_the_first() {
    let mut sequence = armed();
    sequence.input(chord("1"));
    type_text(&mut sequence, "new");
    sequence.input(Input::MarkChord);
    assert_eq!(
        sequence.enter(&["new"]),
        Outcome::Refused(Refusal::Unbound(Binding {
            reference: Reference::Key("1".into()),
            alone: false,
        }))
    );

    let mut word_then_key = armed();
    type_text(&mut word_then_key, "new");
    word_then_key.input(chord("3"));
    assert_eq!(
        word_then_key.enter(&["new"]),
        Outcome::Refused(Refusal::Unbound(Binding {
            reference: Reference::Key("3".into()),
            alone: false,
        })),
        "one unbound key beside a word is not alone"
    );

    let mut lone = armed();
    lone.input(digit("3"));
    assert_eq!(
        lone.enter(&["any"]),
        Outcome::Refused(Refusal::Unbound(Binding {
            reference: Reference::Key("3".into()),
            alone: true,
        }))
    );

    let mut sequence = Sequence::new(bindings(Some("gone"), &[]));
    sequence.input(chord("1"));
    sequence.input(Input::MarkChord);
    assert_eq!(
        sequence.enter(&["new"]),
        Outcome::Refused(Refusal::Missing("gone".into()))
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// Where a refusal goes and its wording
// ---------------------------------------------------------------------------------------------------------------------

/// Outside the launcher, the refusals New shows keep the old app's words, and every word and name in them is a host
/// part, so the drawer escapes it rather than letting a template name pose as the app's own explanation.
#[test]
fn refusals_use_the_old_words_with_names_as_host_parts() {
    let unknown = Refusal::UnknownWord("zz".into()).sentence();
    assert_eq!(plain(&unknown), "No template matches \"zz\"");
    assert_eq!(unknown[1], Part::Host("zz".into()));

    let ambiguous = Refusal::AmbiguousWord {
        word: "co".into(),
        candidates: vec!["codex".into(), "codex-yolo".into()],
    }
    .sentence();
    assert_eq!(
        plain(&ambiguous),
        "Template word \"co\" is ambiguous: codex, codex-yolo"
    );
    let hosts: Vec<_> = ambiguous
        .iter()
        .filter_map(|part| match part {
            Part::Host(text) => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(hosts, ["co", "codex", "codex-yolo"]);

    let missing = Refusal::Missing("gone".into()).sentence();
    assert_eq!(plain(&missing), "Template gone is no longer available");
    assert_eq!(missing[1], Part::Host("gone".into()));
}

/// Outside the launcher, only an unbound reference opens Templates, with the hint for the first unbound reference
/// (the old app's own two hints are one here). Inside the launcher, an unbound key or ⚡ is named when the sequence was
/// that one reference and gets a sentence about the sequence otherwise. A key or ⚡ whose captured template is gone
/// always gets the sentence about the sequence, alone or not, as in the old app, where only unbound references reached
/// the by-name wording; naming it would say "No template is marked ⚡" while ⚡ is still set. Word refusals read as they
/// do outside the launcher.
#[test]
fn key_refusals_choose_a_hint_and_a_launcher_sentence() {
    let key = |alone| Binding {
        reference: Reference::Key("3".into()),
        alone,
    };
    let mark = |alone| Binding {
        reference: Reference::Mark,
        alone,
    };
    let missing = || Refusal::Missing("gone".into());

    assert_eq!(Refusal::Unbound(key(true)).hint(), Some(Hint::NumberKey));
    assert_eq!(
        Refusal::Unbound(mark(false)).hint(),
        Some(Hint::QuickLaunch)
    );
    assert_eq!(missing().hint(), None, "a gone template opens New");
    assert_eq!(Refusal::UnknownWord("zz".into()).hint(), None);

    assert_eq!(
        plain(&Refusal::Unbound(key(true)).launcher_sentence()),
        "No template is bound to key 3"
    );
    assert_eq!(
        plain(&Refusal::Unbound(mark(true)).launcher_sentence()),
        "No template is marked ⚡ for quick launch"
    );
    for refusal in [
        Refusal::Unbound(key(false)),
        Refusal::Unbound(mark(false)),
        missing(),
    ] {
        assert_eq!(
            plain(&refusal.launcher_sentence()),
            "A template in this sequence is unbound or missing",
            "{refusal:?}"
        );
    }
    let unknown = Refusal::UnknownWord("zz".into());
    assert_eq!(unknown.launcher_sentence(), unknown.sentence());
}

//! Tests of the Custom permission words block that the launcher and the template editor share: how its words are named
//! for a screen reader, and that an error replaces them.

use gpui::{IntoElement as _, ParentElement as _, TestAppContext};

use crate::components::CustomFlags;
use crate::test_support::{self, frame};

/// The words' accessible name is what tells `--a 'b c'` (two words) from `--a b c` (three) to a screen reader, and
/// a parse error must replace the words rather than sit beside stale ones. Specified: flags that parse are named
/// "Permission arguments:" with each word escaped and the words apart by commas, with no error node; flags that do
/// not parse draw the capitalized error and no words.
#[gpui::test]
fn the_words_are_named_apart_and_an_error_replaces_them(cx: &mut TestAppContext) {
    let cx = test_support::open(cx, |_, _| {
        gpui::div()
            .child(CustomFlags::new(
                "ok",
                gpui::div(),
                "--a 'b c' \u{202E}x",
                "Codex",
            ))
            .child(CustomFlags::new("bad", gpui::div(), "--a 'open", "Codex"))
            .into_any_element()
    });
    let nodes = frame(cx);
    let label = |id: &str| {
        nodes
            .iter()
            .find(|node| node.element_id.as_deref() == Some(id))
            .and_then(|node| node.label.as_ref().map(|label| label.to_string()))
    };
    assert_eq!(
        label("ok-flag-words").as_deref(),
        Some("Permission arguments: --a, b c, <U+202E>x")
    );
    assert_eq!(label("ok-flags-error"), None);
    assert_eq!(label("bad-flag-words"), None);
    assert!(
        label("bad-flags-error").is_some_and(|error| error.starts_with(char::is_uppercase)),
        "the error is drawn as a sentence"
    );
}

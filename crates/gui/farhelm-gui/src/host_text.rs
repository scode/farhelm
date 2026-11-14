//! Host-supplied text: the one owner of how text a host wrote reaches the screen.
//!
//! SPEC.md (confirmed 2026-10-10): text a host supplied that the GUI shows outside a terminal is escaped so invisible
//! and direction-changing characters are visible, including in accessible names, wherever it appears and however it
//! reached the helm. SPEC_impl.md, "Host-supplied text", makes that a type: a [`HostText`] is made from a host's value
//! where the GUI first reads it, and the only strings that ever come out of it are its escaped presentation (for display
//! and accessible names, produced inside this module and drawn only by this module's elements) and its raw value (for
//! editing and operations, through [`HostText::raw`]). There is no `Display`, `Deref` or `AsRef<str>`, and `Debug`
//! prints the escaped form, so a host's bytes cannot slip into a label, a log line or a tooltip by accident.
//!
//! Escaping uses the one table and notation shared with the rest of Farhelm (`farhelm_proto::text::escaped`): every
//! presentation-unsafe character becomes `<U+XXXX>`, and a literal `<U+` the host typed has its `<` escaped so it cannot
//! pose as one.
//!
//! Direction isolation is structural: every piece of host text is laid out as its own text element, never concatenated
//! into a string with the app's own words. GPUI shapes each text element on its own, so right-to-left letters in a
//! session title reorder only within that title, the way HTML's `<bdi>` isolated them in the old app; the explicit
//! direction controls that could reach further are escaped by the shared table. A sentence that mixes the app's words
//! with host text is built with [`HostSentence`], which keeps every part its own element and exposes the whole to
//! assistive technology as one label whose name is made of the same escaped parts.
//!
//! Terminal output is not host text in this sense: it is the remote program's own display (SPEC.md), drawn by the
//! terminal.
//!
//! Extension points (docs/gpui/foundation-ownership.md): a view that shows a new host-supplied field wraps it with
//! [`HostText::from_host`] where it reads the field and draws it with [`element`] (or [`marked`], for a value with some
//! characters marked) or a [`HostSentence`]; a component that takes host text takes `&HostText` and draws it the same
//! way, or, for wording that is usually the app's own but may name a host value (a dialog's summary, a prompt's lines),
//! takes a [`Line`]. Identity labels the user compares (installation ids, which SPEC.md also wants with their spaces
//! visible) are not built yet; they arrive with the first view that shows one, as a second presentation here.

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    AnyElement, App, Div, ElementId, HighlightStyle, InteractiveElement as _, IntoElement,
    ParentElement, RenderOnce, Role, SharedString, StatefulInteractiveElement as _, Styled,
    StyledText, Window, div,
};

/// Text a host supplied, as the GUI holds it from the moment it reads it.
///
/// Holds the raw value and its escaped presentation, computed once. Equality and hashing are defined on the raw value:
/// two different host values are different values even when they look alike on screen (a homoglyph, say), and a later
/// presentation that drops information (truncation, say) must not make two values equal.
#[derive(Clone)]
pub struct HostText {
    raw: Arc<str>,
    presentation: SharedString,
}

impl HostText {
    /// Take a host-supplied value. Call this where the GUI first reads a host-authored field (a session title, a host
    /// name, a model name, an error message relayed from a supervisor), not later at drawing time.
    pub fn from_host(raw: impl Into<Arc<str>>) -> Self {
        let raw = raw.into();
        let presentation = SharedString::from(farhelm_proto::text::escaped(&raw));
        HostText { raw, presentation }
    }

    /// The value exactly as the host wrote it, for editing (a rename draft starts from it) and for operations (it goes
    /// back to the helm byte for byte). Never for display: an escaped presentation never becomes a saved value, and a
    /// raw value never becomes a label.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The escaped presentation, for this module's elements only.
    fn presentation(&self) -> SharedString {
        self.presentation.clone()
    }
}

impl PartialEq for HostText {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl Eq for HostText {}

impl std::hash::Hash for HostText {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

/// Prints the escaped presentation, so a host value in a log or test message is as visible as on screen.
impl fmt::Debug for HostText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HostText")
            .field(&self.presentation.as_ref())
            .finish()
    }
}

/// Draw `text` on its own: its escaped presentation in a text element of its own, so its direction stays inside it.
/// It inherits the surrounding text style (font, size, color), as any label does.
pub fn element(text: &HostText) -> Div {
    div().child(text.presentation())
}

/// Draw `text` like [`element`], with some of its characters drawn in `style`: the quick switcher's matched characters
/// (SPEC.md, "Desktop window chrome": rows show their values "with matching characters marked"; the old switcher drew
/// them in the accent color at weight 600).
///
/// `marks` is handed the escaped presentation, the spelling the user sees, and returns the char indices to mark in it.
/// Matching on that spelling keeps presentation from changing which characters look matched: a raw match whose escaped
/// form does not match shows no mark. An index inside a `<U+…>` escape is dropped, so no escape is ever drawn partly
/// marked, and an index past the end is dropped too; a matcher that settles on a hit inside an escape therefore marks
/// less, rather than moving on to a later visible hit. The value stays one text element, so its direction stays
/// isolated, and it has no accessible name of its own, as [`element`] has none: the marks are drawn, not announced.
///
/// The presentation reaches `marks` only to be matched against: a caller must not keep it or draw it, which would put a
/// host value on screen outside this module's elements (the one rule of SPEC_impl.md, "Host-supplied text").
pub fn marked(
    text: &HostText,
    marks: impl FnOnce(&str) -> Vec<usize>,
    style: HighlightStyle,
) -> Div {
    let presentation = text.presentation();
    let ranges = mark_ranges(&presentation, marks(&presentation));
    div().child(
        StyledText::new(presentation)
            .with_highlights(ranges.into_iter().map(|range| (range, style))),
    )
}

/// The byte ranges of `presentation` to draw marked, for the char indices `chosen`: in order, adjacent characters
/// joined into one range, and nothing inside an escape or past the end.
fn mark_ranges(presentation: &str, mut chosen: Vec<usize>) -> Vec<Range<usize>> {
    chosen.sort_unstable();
    chosen.dedup();
    // Every `<U+` in a presentation starts an escape (a literal `<U+` a host typed has its `<` escaped), running to the
    // next `>`.
    let escapes: Vec<Range<usize>> = presentation
        .match_indices("<U+")
        .filter_map(|(start, _)| {
            presentation[start..]
                .find('>')
                .map(|end| start..start + end + 1)
        })
        .collect();
    let chars: Vec<(usize, char)> = presentation.char_indices().collect();
    let mut ranges: Vec<Range<usize>> = Vec::new();
    for index in chosen {
        let Some(&(start, ch)) = chars.get(index) else {
            continue;
        };
        if escapes.iter().any(|escape| escape.contains(&start)) {
            continue;
        }
        let end = start + ch.len_utf8();
        match ranges.last_mut() {
            Some(last) if last.end == start => last.end = end,
            _ => ranges.push(start..end),
        }
    }
    ranges
}

/// One part of a [`HostSentence`].
#[derive(Clone)]
enum Part {
    /// The app's own words.
    App(SharedString),
    /// A host's value.
    Host(HostText),
}

/// A line that mixes the app's own words with host-supplied text ("Delete «api-refactor» on «build-box»?"), drawn as a
/// row of parts in which every host part is its own text element, and exposed to assistive technology (and to the
/// agent bridge's element list) as one label node under its id, named by [`HostSentence::accessible_name`].
///
/// The parts are never concatenated for display, which is what keeps a right-to-left host value from reordering the
/// app's words around it. The parts themselves carry no id, so they are not nodes of their own: a screen reader reads
/// the sentence once, by its name, rather than as a run of fragments.
///
/// A clone draws the same parts under the same id, so two clones drawn in one frame are two nodes with one id; a
/// sentence is cloned to be kept (inside a frozen [`Question`](crate::prompt::Question), say), not to be drawn twice.
#[derive(Clone, IntoElement)]
pub struct HostSentence {
    id: ElementId,
    parts: Vec<Part>,
}

impl HostSentence {
    /// An empty sentence drawn under `id`.
    pub fn new(id: impl Into<ElementId>) -> Self {
        HostSentence {
            id: id.into(),
            parts: Vec::new(),
        }
    }

    /// Append the app's own words.
    pub fn text(mut self, words: impl Into<SharedString>) -> Self {
        self.parts.push(Part::App(words.into()));
        self
    }

    /// Append a host's value.
    pub fn host(mut self, text: &HostText) -> Self {
        self.parts.push(Part::Host(text.clone()));
        self
    }

    /// The sentence as assistive technology reads it: the app's words as they are and each host value escaped, so an
    /// accessible name shows a hidden character exactly as the screen does (SPEC.md: "including in accessible names").
    pub fn accessible_name(&self) -> SharedString {
        let mut name = String::new();
        for part in &self.parts {
            match part {
                Part::App(words) => name.push_str(words),
                Part::Host(text) => name.push_str(&text.presentation()),
            }
        }
        name.into()
    }
}

impl RenderOnce for HostSentence {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let name = self.accessible_name();
        // GPUI puts an element in the accessibility tree (and the bridge's record) only when it declares a role; a
        // div's label alone is dropped. `Label` is the role GPUI gives its own id-bearing text.
        div()
            .id(self.id)
            .role(Role::Label)
            .aria_label(name)
            .flex()
            .flex_row()
            .flex_wrap()
            // `min_w_0` lets a part shrink to the row and wrap. Without it a flex item's minimum width is its whole
            // unwrapped line (GPUI measures text with no wrap width for that query), so a long session title in a
            // dialog summary ran past the dialog's edge and was cut off there, hiding the part that tells two
            // look-alike titles apart. Parts still wrap as whole blocks, since each is its own element.
            .children(self.parts.into_iter().map(|part| -> AnyElement {
                match part {
                    Part::App(words) => div().min_w_0().child(words).into_any_element(),
                    Part::Host(text) => element(&text).min_w_0().into_any_element(),
                }
            }))
    }
}

/// One line of wording a component shows: the app's own words, or a [`HostSentence`] that names a host value.
///
/// Components whose text is usually the app's own but sometimes names something a host wrote (a dialog's summary, a
/// prompt's lines: "Delete «api-refactor» on «build-box»?") take a `Line`, so a caller never has to turn a host value
/// into a string to fit a string parameter. Without it, the only way in would be formatting [`HostText::raw`] into a
/// label, which is exactly the unescaped, unisolated path SPEC_impl.md, "Host-supplied text", rules out. Plain strings
/// convert through `From`, so app wording reads as it always did (`.summary("On this machine")`).
///
/// An app line draws exactly as a string child would. A sentence draws through [`HostSentence`]'s own element: one
/// labelled node, under the id the caller gave it, named with its host parts escaped. Titles and button labels stay
/// plain strings, for the app's own words only: a host value goes in the summary or a line, never in a dialog's
/// accessible name. That is a convention, not something the type enforces, since a title still takes any string.
#[derive(Clone, IntoElement)]
pub enum Line {
    /// The app's own words.
    App(SharedString),
    /// A sentence mixing the app's words with host values.
    Sentence(HostSentence),
}

impl From<&'static str> for Line {
    fn from(words: &'static str) -> Self {
        Line::App(words.into())
    }
}

impl From<String> for Line {
    fn from(words: String) -> Self {
        Line::App(words.into())
    }
}

impl From<SharedString> for Line {
    fn from(words: SharedString) -> Self {
        Line::App(words)
    }
}

impl From<HostSentence> for Line {
    fn from(sentence: HostSentence) -> Self {
        Line::Sentence(sentence)
    }
}

impl RenderOnce for Line {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        match self {
            Line::App(words) => words.into_any_element(),
            Line::Sentence(sentence) => sentence.into_any_element(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Spoofing strings a host could put in a session title or host name, each paired with what the GUI must show.
    /// Kept beside the type so every view that renders host text can push the same corpus through itself.
    pub(crate) const SPOOFING_CORPUS: &[(&str, &str)] = &[
        // A right-to-left override that would reverse the text after it.
        ("invoice\u{202E}fdp.exe", "invoice<U+202E>fdp.exe"),
        // An isolate and a mark that would reorder the surrounding sentence.
        ("\u{2067}admin\u{2069}", "<U+2067>admin<U+2069>"),
        ("a\u{200F}b", "a<U+200F>b"),
        // Zero-width characters that make two names look identical.
        ("pay\u{200B}pal", "pay<U+200B>pal"),
        ("bu\u{00AD}ild", "bu<U+00AD>ild"),
        ("x\u{FEFF}y", "x<U+FEFF>y"),
        // A newline that would forge a second line, and a terminal escape.
        ("one\ntwo", "one<U+000A>two"),
        ("\u{1b}[31mred", "<U+001B>[31mred"),
        // A format character outside the classic invisible set (a musical symbol format control).
        ("beat\u{1D173}", "beat<U+1D173>"),
        // Literal escape notation that would pose as an escape the app made.
        ("<U+202E>", "<U+003C>U+202E>"),
        // Ordinary text a host may legitimately write is kept as it is.
        ("api-refactor", "api-refactor"),
        (
            "caf\u{e9} \u{65e5}\u{672c} \u{2764}\u{FE0F}",
            "caf\u{e9} \u{65e5}\u{672c} \u{2764}\u{FE0F}",
        ),
        ("  spaced  out  ", "  spaced  out  "),
        (
            "\u{5e9}\u{5dc}\u{5d5}\u{5dd}",
            "\u{5e9}\u{5dc}\u{5d5}\u{5dd}",
        ),
    ];

    /// Why this matters: SPEC.md requires every host value the GUI shows outside a terminal to have its hidden and
    /// direction-changing characters visible, and the old app's bugs here were values reaching a label without going
    /// through the escape. The presentation is the only displayable form a `HostText` has, so it carries the whole
    /// guarantee.
    ///
    /// Specification: for every string in the spoofing corpus, the presentation is exactly the expected escaped
    /// text, the raw value is the host's bytes unchanged, and `Debug` shows the escaped form, never the raw one.
    #[test]
    fn every_spoofing_string_is_shown_escaped_and_kept_raw_for_editing() {
        for (raw, shown) in SPOOFING_CORPUS {
            let text = HostText::from_host(*raw);
            assert_eq!(
                text.presentation().as_ref(),
                *shown,
                "presentation of {raw:?}"
            );
            assert_eq!(
                text.raw(),
                *raw,
                "raw value of {shown:?} must keep the host's bytes"
            );
            assert_eq!(format!("{text:?}"), format!("HostText({shown:?})"));
        }
    }

    /// Why this matters: an accessible name is a display surface too (SPEC.md: "including in accessible names"), and a
    /// sentence that named its host parts raw would hand a screen reader the override the screen hides.
    ///
    /// Specification: a sentence's accessible name is its parts in order, the app's words as written and each host
    /// value in its escaped presentation; it never contains a presentation-unsafe character.
    #[test]
    fn a_sentence_is_named_with_its_host_parts_escaped() {
        let title = HostText::from_host("invoice\u{202E}fdp.exe");
        let host = HostText::from_host("build\u{200B}box");
        let sentence = HostSentence::new("delete-question")
            .text("Delete ")
            .host(&title)
            .text(" on ")
            .host(&host)
            .text("?");
        let name = sentence.accessible_name();
        assert_eq!(
            name.as_ref(),
            "Delete invoice<U+202E>fdp.exe on build<U+200B>box?"
        );
        assert!(
            !name
                .chars()
                .any(farhelm_proto::text::is_presentation_unsafe)
        );
    }

    /// Draws one delete question as a view would, so the test below can read what the drawn frame recorded.
    struct SentenceView;

    impl gpui::Render for SentenceView {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div().child(
                HostSentence::new("delete-question")
                    .text("Delete ")
                    .host(&HostText::from_host("invoice\u{202E}fdp.exe"))
                    .text("?"),
            )
        }
    }

    /// Why this matters: GPUI keeps an element out of the accessibility tree unless it declares a role, and drops its
    /// label with it, so a sentence could compute the right name and still reach a screen reader with none (the first
    /// draft of this type did exactly that). The agent bridge reads the same per-frame record, so the sentence is also
    /// what an automated check of a view finds.
    ///
    /// Specification: with GPUI's driver record on, a drawn sentence is a node under its own id with the `Label` role
    /// and its escaped accessible name as the label, and none of its parts is a node with a role of its own.
    #[gpui::test]
    fn a_drawn_sentence_is_one_labelled_node_with_its_escaped_name(cx: &mut gpui::TestAppContext) {
        crate::test_support::install(cx);
        let (_, cx) = cx.add_window_view(|_, _| SentenceView);
        cx.update(|window, _| window.set_driver_enabled(true));
        cx.run_until_parked();
        let nodes = cx.update(|window, _| window.driver_nodes().to_vec());
        let Some(sentence) = nodes
            .iter()
            .position(|node| node.element_id.as_deref() == Some("delete-question"))
        else {
            let ids: Vec<_> = nodes.iter().map(|node| &node.element_id).collect();
            panic!("the sentence was not recorded; recorded ids: {ids:?}");
        };
        assert_eq!(nodes[sentence].role, Some(Role::Label));
        assert_eq!(
            nodes[sentence].label.as_deref(),
            Some("Delete invoice<U+202E>fdp.exe?")
        );
        // Every node recorded below the sentence, at any depth: the part wrappers and their text.
        let inside = |mut index: usize| loop {
            match nodes[index].parent {
                Some(parent) if parent == sentence => return true,
                Some(parent) => index = parent,
                None => return false,
            }
        };
        let parts: Vec<_> = (0..nodes.len()).filter(|&index| inside(index)).collect();
        assert!(!parts.is_empty(), "the sentence's parts were not recorded");
        assert!(
            parts.iter().all(|&index| nodes[index].role.is_none()),
            "a part of the sentence has a role of its own"
        );
    }

    /// Why this matters: the quick switcher marks the characters a search matched in host values, and the marks are
    /// the only part of the value drawn differently, so they must cover exactly the matched characters.
    ///
    /// Specification: in a plain value, marked char indices become the byte ranges of those characters, in order, with
    /// neighbours joined, duplicates ignored and indices past the end dropped.
    #[test]
    fn marks_in_a_plain_value_cover_exactly_the_matched_characters() {
        let shown = HostText::from_host("api-refactor").presentation();
        assert_eq!(
            mark_ranges(&shown, vec![4, 0, 1, 1, 5, 99]),
            vec![0..2, 4..6]
        );
    }

    /// Why this matters: an escape such as `<U+202E>` is how a hidden character is shown; drawing part of it marked
    /// would split it visually and make a matcher's hit on the notation's letters look like a hit on the value.
    ///
    /// Specification: the matcher is handed the escaped spelling; in a value whose presentation holds escapes
    /// (including the escape of a literal `<U+` the host typed), indices inside an escape are dropped and indices around
    /// it are kept, at their byte positions.
    #[test]
    fn a_mark_never_splits_an_escape() {
        let mut handed = String::new();
        let _ = marked(
            &HostText::from_host("a\u{202E}b"),
            |shown| {
                handed = shown.to_owned();
                Vec::new()
            },
            HighlightStyle::default(),
        );
        assert_eq!(
            handed, "a<U+202E>b",
            "the matcher was handed the escaped spelling"
        );
        let shown = HostText::from_host("a\u{202E}b").presentation();
        assert_eq!(mark_ranges(&shown, (0..10).collect()), vec![0..1, 9..10]);
        let literal = HostText::from_host("<U+202E>").presentation();
        assert_eq!(literal.as_ref(), "<U+003C>U+202E>");
        assert_eq!(
            mark_ranges(&literal, (0..15).collect()),
            vec![8..15],
            "only the host's own letters after the escaped `<` are markable"
        );
    }

    /// Draws one marked right-to-left value beside the app's words, as a switcher row would.
    struct MarkedView;

    impl gpui::Render for MarkedView {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div()
                .id("row")
                .role(Role::Label)
                .aria_label("Open")
                .flex()
                .child("Open ")
                .child(marked(
                    &HostText::from_host("\u{5e9}\u{5dc}\u{5d5}\u{5dd}"),
                    |_| vec![1, 2],
                    HighlightStyle {
                        font_weight: Some(gpui::FontWeight::SEMIBOLD),
                        ..HighlightStyle::default()
                    },
                ))
        }
    }

    /// Why this matters: right-to-left letters in a host value must reorder only within that value (SPEC_impl.md,
    /// "Host-supplied text"), which holds while the value is its own text element; and a mark's range must fall on
    /// character boundaries of multi-byte text, or drawing it fails.
    ///
    /// Specification: in a right-to-left value of two-byte letters, char indices map to two-byte ranges; drawn beside
    /// the app's words with two letters marked at a heavier weight, the value is painted as one whole line of its own,
    /// never as pieces split at the marks, and adds no node with a role or a name of its own.
    #[gpui::test]
    fn a_marked_right_to_left_value_stays_one_isolated_element(cx: &mut gpui::TestAppContext) {
        let value = "\u{5e9}\u{5dc}\u{5d5}\u{5dd}";
        let shown = HostText::from_host(value).presentation();
        assert_eq!(mark_ranges(&shown, vec![1, 2]), vec![2..6]);
        crate::test_support::install(cx);
        let (_, cx) = cx.add_window_view(|_, _| MarkedView);
        cx.update(|window, _| window.set_driver_enabled(true));
        cx.run_until_parked();
        let nodes = cx.update(|window, _| window.driver_nodes().to_vec());
        let row = nodes
            .iter()
            .position(|node| node.element_id.as_deref() == Some("row"))
            .expect("the row was drawn");
        let inside = |mut index: usize| loop {
            match nodes[index].parent {
                Some(parent) if parent == row => return true,
                Some(parent) => index = parent,
                None => return false,
            }
        };
        let painted: Vec<String> = (0..nodes.len())
            .filter(|&index| inside(index))
            .flat_map(|index| nodes[index].texts.iter().map(|text| text.text.to_string()))
            .filter(|text| text.chars().any(|ch| value.contains(ch)))
            .collect();
        assert_eq!(
            painted,
            vec![value.to_string()],
            "the value was painted whole, as one line"
        );
        assert!(
            (0..nodes.len())
                .filter(|&index| inside(index))
                .all(|index| nodes[index].role.is_none() && nodes[index].label.is_none()),
            "the marked value adds no node with a role or name of its own"
        );
    }

    /// Why this matters: equality decides whether a pushed change touched a value (a draft, a selection), and two host
    /// values that differ only by a hidden character are exactly the ones that must not be treated as the same.
    ///
    /// Specification: a value and its look-alike with a zero-width space are different values, and a value equals
    /// itself. Equality is on the raw bytes by definition (the `PartialEq` impl), and this test cannot tell that apart
    /// from comparing presentations: escaping is injective (every `<U+` in a presentation is an escape, so it decodes
    /// back to one raw value), so no pair of inputs differs in one and not the other. It pins the outcome, not the
    /// mechanism.
    #[test]
    fn look_alike_values_are_different_values() {
        assert_ne!(
            HostText::from_host("paypal"),
            HostText::from_host("pay\u{200B}pal")
        );
        assert_eq!(HostText::from_host("paypal"), HostText::from_host("paypal"));
    }
}

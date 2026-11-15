//! Host-supplied text: the one owner of how text a host wrote reaches the screen.
//!
//! SPEC.md (confirmed 2026-10-10): text a host supplied that the GUI shows outside a terminal is escaped so invisible
//! and direction-changing characters are visible, including in accessible names, wherever it appears and however it
//! reached the helm. SPEC_impl.md, "Host-supplied text", makes that a type: a [`HostText`] is made from a host's value
//! where the GUI first reads it, and the only strings that ever come out of it are its escaped spellings (the
//! presentation for display and the accessible spelling for accessible names, the same string except for an abbreviated
//! identity; produced inside this module and drawn only by this module's elements) and its raw value (for editing and
//! operations, through [`HostText::raw`]). There is no `Display`, `Deref` or `AsRef<str>`, and `Debug` prints the
//! escaped form, so a host's bytes cannot slip into a label, a log line or a tooltip by accident.
//!
//! Escaping uses the one table and notation shared with the rest of Farhelm (`farhelm_proto::text::escaped`): every
//! presentation-unsafe character becomes `<U+XXXX>`, and a literal `<U+` the host typed has its `<` escaped so it cannot
//! pose as one.
//!
//! Direction isolation comes in two shapes. A host value drawn alone ([`element`], [`marked`]) is its own text element,
//! which GPUI shapes as a bidirectional paragraph of its own, so right-to-left letters in a session title reorder only
//! within that title. A sentence that mixes the app's words with host text ([`HostSentence`]) is one text element, so it
//! wraps at word boundaries like the old app's inline text, and each host value in it sits inside a left-to-right
//! isolate (U+2066 … U+2069), the Unicode form of the old app's `dir="ltr"` `unicode-bidi: isolate` spans. The isolate is
//! load-bearing: GPUI applies the bidirectional algorithm to a whole text element across its runs (cosmic-text on Linux,
//! one CoreText line on macOS), so without it a right-to-left value would reorder the app's words around it, and a
//! sentence that starts with one would turn right-to-left as a whole. The isolate holds because the shared table escapes
//! every direction control and paragraph separator a host could write, so no host value can close its isolate early or
//! end the paragraph. Host text never meets the app's words in a string any other way. The sentence is exposed to
//! assistive technology as one label whose name is made of the same escaped parts, without the isolates. One
//! consequence the old app had too: a value that mixes directions ("אבג abc") takes its direction from its own first
//! letter when drawn alone, but is always left-to-right in a sentence, so its two halves can trade places between a
//! row and a dialog that names it.
//!
//! Typography is separate from trust. A technical host value inside a sentence (a destination, an executable path, a
//! version) is set in the monospace face with [`HostSentence::mono`], as the old app's `MonoPeer` runs were, and is
//! escaped, isolated and named exactly as any other host part; only the drawn face differs. Names and host-written
//! prose keep the surrounding interface face ([`HostSentence::host`]).
//!
//! Terminal output is not host text in this sense: it is the remote program's own display (SPEC.md), drawn by the
//! terminal.
//!
//! Extension points (docs/gpui/foundation-ownership.md): a view that shows a new host-supplied field wraps it with
//! [`HostText::from_host`] where it reads the field and draws it with [`element`] (or [`marked`], for a value with some
//! characters marked) or a [`HostSentence`]; a component that takes host text takes `&HostText` and draws it the same
//! way, or, for wording that is usually the app's own but may name a host value (a dialog's title and summary, a
//! prompt's lines), takes a [`Line`]. A sentence's technical host values go in with [`HostSentence::mono`] rather than
//! by restyling a part from outside, so the escaping and isolation stay this module's. A control's hover help that
//! names a host value is a [`HostSentence`] handed to `HoverHelp::sentence` (or several, one per paragraph, handed to
//! `HoverHelp::paragraphs`): the tooltip draws each through this module's element, and the control's accessible
//! description is the sentence's [`HostSentence::accessible_name`] (for paragraphs, each one's, a blank line apart).
//! An identity the user compares (an installation id, a session id: SPEC.md wants their spaces visible too) is made
//! with [`HostText::identity`] instead of [`HostText::from_host`], or with [`HostText::identity_abbreviated`] where a
//! compact line shows only its ends. An abbreviated identity is drawn short and named in full, since SPEC.md's From
//! line wants the shortened ID's accessible name to carry the complete one; hover help that shows the full ID is still
//! the view's to add, through [`HostText::identity`].

use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use gpui::{
    App, Div, ElementId, HighlightStyle, InteractiveElement as _, IntoElement, ParentElement,
    RenderOnce, Role, SharedString, StatefulInteractiveElement as _, Styled, StyledText, Window,
    div,
};

use crate::assets;

/// Text a host supplied, as the GUI holds it from the moment it reads it.
///
/// Holds the raw value and two spellings of it, computed once and chosen by the constructor: the presentation, which is
/// drawn, and the accessible spelling, which names it (in a sentence's accessible name, and in `Debug`). For ordinary
/// values ([`HostText::from_host`]) both are the escaped text, and for an identity ([`HostText::identity`]) both are its
/// stricter spelling. Only [`HostText::identity_abbreviated`] makes them differ: drawn short, named in full. A view
/// never picks a spelling at drawing time.
///
/// Equality compares the raw value and both spellings: two different host values are different values even when they
/// look alike on screen (a homoglyph, say), and two presentations of one value are different values because they draw
/// differently (a hover help that switches between them must count as changed). Hashing uses the raw value alone, which
/// keeps equal values hashing equal.
#[derive(Clone)]
pub struct HostText {
    raw: Arc<str>,
    presentation: SharedString,
    accessible: SharedString,
}

impl HostText {
    /// Take a host-supplied value. Call this where the GUI first reads a host-authored field (a session title, a host
    /// name, a model name, an error message relayed from a supervisor), not later at drawing time.
    pub fn from_host(raw: impl Into<Arc<str>>) -> Self {
        let raw = raw.into();
        let presentation = SharedString::from(farhelm_proto::text::escaped(&raw));
        HostText::drawn_as_named(raw, presentation)
    }

    /// Take a host-supplied identity the user compares with another (an installation id in a "recorded as install X;
    /// now reports Y" question, a session id on an approval card), shown so that two different identities never look
    /// alike.
    ///
    /// Printable ASCII is shown as itself except `<`, `(` and `)`; every other character, spaces included, is written
    /// as `<U+XXXX>`, and the empty identity is shown as `(empty)`. Escaping `<` means every `<` on screen starts an
    /// escape, so a literal `<U+0020>` cannot pose as a space; escaping the parentheses keeps `(empty)` for the empty
    /// value alone. UUIDs, domain names and IP addresses still read as they are. Unlike [`from_host`](Self::from_host),
    /// this also spells variation selectors and non-ASCII letters, which the shared table leaves alone so ordinary
    /// titles keep rendering; in a comparison, a look-alike letter shown as itself would pass for the one it imitates.
    /// Ported from the old app's `display_identity`.
    pub fn identity(raw: impl Into<Arc<str>>) -> Self {
        let raw = raw.into();
        let presentation = SharedString::from(identity_spelling(&raw));
        HostText::drawn_as_named(raw, presentation)
    }

    /// Take a host-supplied identity for a compact line that shows only its start and end (SPEC.md: the approval
    /// card's From line shows "the start and end of the requester's bare ID, without splitting an escape spelling").
    ///
    /// A full session reference (entry UUID, colon, supervisor id) is shortened from its bare id: exactly one valid
    /// entry prefix is dropped, and a bare id that itself looks like a full reference keeps its look-alike prefix as
    /// data. Anything that does not parse as a reference is shortened whole. The shown value is the bare id in
    /// [`identity`](Self::identity)'s spelling, kept whole when that spelling is 16 characters or fewer, and otherwise
    /// cut to about its first 8 and last 4 shown characters around `…`. The cuts fall only between whole per-character
    /// spellings, so an escape at a cut is kept whole and the kept ends may run longer than 8 and 4.
    ///
    /// Only the drawing is short. The accessible spelling is [`identity`](Self::identity)'s spelling of the whole
    /// reference, entry prefix included, so a sentence holding this part is named with the complete ID (SPEC.md: the
    /// shortened ID's "hover and accessible name carry the complete entry-prefixed ID"), as the old card's label was.
    /// The hover is not this type's: a view that shows the full ID there builds its help from an
    /// [`identity`](Self::identity) of the same raw value. The raw value is the whole reference too, so a copy or an
    /// operation gets every byte. Ported from the old app's `shortened_requester_id`.
    pub fn identity_abbreviated(raw: impl Into<Arc<str>>) -> Self {
        let raw = raw.into();
        HostText {
            presentation: SharedString::from(abbreviated_identity_spelling(&raw)),
            accessible: SharedString::from(identity_spelling(&raw)),
            raw,
        }
    }

    /// A value named exactly as it is drawn, which is every constructor's but the abbreviation's.
    fn drawn_as_named(raw: Arc<str>, presentation: SharedString) -> Self {
        HostText {
            raw,
            accessible: presentation.clone(),
            presentation,
        }
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

    /// The escaped spelling that names the value, for accessible names and `Debug` only.
    fn accessible(&self) -> SharedString {
        self.accessible.clone()
    }
}

impl PartialEq for HostText {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
            && self.presentation == other.presentation
            && self.accessible == other.accessible
    }
}

impl Eq for HostText {}

impl std::hash::Hash for HostText {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.raw.hash(state);
    }
}

/// Prints the accessible spelling, so a host value in a log or test message is as visible as on screen, and an
/// abbreviated identity shows in full.
impl fmt::Debug for HostText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HostText")
            .field(&self.accessible.as_ref())
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

/// The spelling of one identity character: itself for printable ASCII other than `<`, `(` and `)`, else its escape.
/// Every raw character has exactly one token, which is what lets an abbreviation cut between tokens without ever
/// splitting an escape.
fn identity_token(ch: char) -> String {
    if ch.is_ascii_graphic() && !matches!(ch, '<' | '(' | ')') {
        ch.to_string()
    } else {
        farhelm_proto::text::notation(ch)
    }
}

/// [`HostText::identity`]'s presentation. Only the empty identity needs the placeholder: a blank one is never blank
/// on screen, since every space has a spelling of its own.
fn identity_spelling(raw: &str) -> String {
    if raw.is_empty() {
        return "(empty)".to_string();
    }
    raw.chars().map(identity_token).collect()
}

/// [`HostText::identity_abbreviated`]'s presentation.
fn abbreviated_identity_spelling(raw: &str) -> String {
    let reference = raw.parse::<farhelm_proto::SessionRef>().ok();
    let bare = reference
        .as_ref()
        .map_or(raw, farhelm_proto::SessionRef::bare_id);
    let tokens: Vec<String> = bare.chars().map(identity_token).collect();
    // Budgets count shown characters, not raw ones: eight invisible characters would otherwise become eighty shown
    // ones. Tokens are ASCII, so their byte length is their shown width.
    if tokens.iter().map(String::len).sum::<usize>() <= 16 {
        return identity_spelling(bare);
    }
    let mut start = 0;
    let mut width = 0;
    while start < tokens.len() && width < 8 {
        width += tokens[start].len();
        start += 1;
    }
    let mut end = tokens.len();
    width = 0;
    while end > start && width < 4 {
        end -= 1;
        width += tokens[end].len();
    }
    // The two ends met, so nothing would be left out: show it all rather than an ellipsis that hides nothing.
    if start == end {
        return tokens.concat();
    }
    format!("{}…{}", tokens[..start].concat(), tokens[end..].concat())
}

/// One part of a [`HostSentence`]. `Debug` prints a host part through [`HostText`]'s own `Debug`, escaped.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    /// The app's own words.
    App(SharedString),
    /// A host's value, in the surrounding face.
    Host(HostText),
    /// A host's value in the monospace face: a [`Part::Host`] in every respect but the face it is drawn in.
    Mono(HostText),
}

/// A line that mixes the app's own words with host-supplied text ("Delete «api-refactor» on «build-box»?"), drawn as
/// one text element that wraps like a sentence, and exposed to assistive technology (and to the agent bridge's element
/// list) as one label node under its id, named by [`HostSentence::accessible_name`].
///
/// One text element is what lets the sentence break between words wherever the line runs out, with no wrapped line
/// starting on the space after a host value; a row of one element per part could only wrap whole parts, so it broke
/// early at part boundaries. Each host value is drawn as its escaped presentation inside a left-to-right isolate, which
/// is what keeps a right-to-left value from reordering the app's words around it (the module docs say why that holds).
/// The drawn string is never named: the accessible name has no isolates. The text carries no id of its own, so a screen
/// reader reads the sentence once, by its name.
///
/// A clone draws the same parts under the same id, so two clones drawn in one frame are two nodes with one id; a
/// sentence is cloned to be kept (inside a frozen [`Question`](crate::prompt::Question), say), not to be drawn twice.
///
/// Equality compares the id and every part, host parts as [`HostText`]'s own equality does (raw value and spellings), so
/// two sentences that differ only by a hidden character in a host value are different; `Debug` prints host parts
/// escaped.
#[derive(Clone, Debug, PartialEq, Eq, IntoElement)]
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

    /// Append a host's value, drawn in the sentence's own face. For names and host-written prose.
    pub fn host(mut self, text: &HostText) -> Self {
        self.parts.push(Part::Host(text.clone()));
        self
    }

    /// Append a technical host value (a destination, a path, a version), drawn in the monospace face, the
    /// terminal's face, at the sentence's size and color, as the old app set its `MonoPeer` runs in `--font-mono`.
    ///
    /// Only the face differs from [`host`](Self::host): the part is the same escaped presentation in the same isolate,
    /// drawn as a run of the sentence's one text element, so its direction stays isolated, and the accessible name (and
    /// so a hover help's description) names it escaped exactly as a host part. Equality still tells the two apart,
    /// since the sentences draw differently.
    pub fn mono(mut self, text: &HostText) -> Self {
        self.parts.push(Part::Mono(text.clone()));
        self
    }

    /// The sentence as assistive technology reads it: the app's words as they are and each host value escaped, so an
    /// accessible name shows a hidden character exactly as the screen does (SPEC.md: "including in accessible names").
    /// A host value is named by its accessible spelling, which is what is drawn except for an abbreviated identity,
    /// named in full ([`HostText::identity_abbreviated`]).
    pub fn accessible_name(&self) -> SharedString {
        let mut name = String::new();
        for part in &self.parts {
            match part {
                Part::App(words) => name.push_str(words),
                Part::Host(text) | Part::Mono(text) => name.push_str(&text.accessible()),
            }
        }
        name.into()
    }

    /// The string the sentence draws, and the byte ranges in it set in the monospace face: the app's words as they
    /// are, and every host value as its escaped presentation inside [`LRI`] … [`PDI`]. Drawing only, never a name or a
    /// value.
    ///
    /// The isolate opens before any spaces the app's words end with, so those spaces sit inside it, still spaces in
    /// the same place. GPUI keeps a line break only where a glyph starts, and on macOS an isolate opened right after
    /// the space took away the break before the value and wrapped the word before it onto the value's line (CoreText,
    /// it appears, gives GPUI no glyph at the isolate). A space has a glyph on every platform.
    ///
    /// Every host part, whatever its face, goes through this one arm, so the isolate cannot be kept on one kind of
    /// host part and dropped from the other. A monospace range covers the value and its closing isolate, not the spaces
    /// before it, which keep the sentence's face.
    fn drawn(&self) -> (SharedString, Vec<Range<usize>>) {
        let mut text = String::new();
        let mut mono = Vec::new();
        for part in &self.parts {
            let value = match part {
                Part::App(words) => {
                    text.push_str(words);
                    continue;
                }
                Part::Host(value) | Part::Mono(value) => value,
            };
            text.insert(text.trim_end_matches(' ').len(), LRI);
            let start = text.len();
            text.push_str(&value.presentation());
            text.push(PDI);
            if matches!(part, Part::Mono(_)) {
                mono.push(start..text.len());
            }
        }
        (text.into(), mono)
    }
}

/// LEFT-TO-RIGHT ISOLATE, which opens a host part, with the spaces before it, in a sentence's drawn string
/// ([`HostSentence::drawn`]). It and [`PDI`] are default-ignorable, so the shaper draws them as nothing.
const LRI: char = '\u{2066}';
/// POP DIRECTIONAL ISOLATE, which closes the isolate [`LRI`] opened. A host value cannot hold one of its own, since
/// the shared table escapes it.
const PDI: char = '\u{2069}';

impl RenderOnce for HostSentence {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let name = self.accessible_name();
        let (text, mono) = self.drawn();
        // GPUI applies a font family override only to runs that lie wholly inside its range, and a text with no
        // highlights is one run, so every monospace range is also handed in as a highlight that changes nothing, which
        // splits the runs at its edges. Both resolve against the inherited style at layout, so size and color still
        // come from the surrounding text.
        let text = StyledText::new(text)
            .with_highlights(
                mono.iter()
                    .map(|range| (range.clone(), HighlightStyle::default())),
            )
            .with_font_family_overrides(
                mono.into_iter()
                    .map(|range| (range, SharedString::from(assets::TERMINAL_FONT))),
            );
        // GPUI puts an element in the accessibility tree (and the bridge's record) only when it declares a role; a
        // div's label alone is dropped. `Label` is the role GPUI gives its own id-bearing text.
        div()
            .id(self.id)
            .role(Role::Label)
            .aria_label(name)
            // `min_w_0` lets the sentence shrink to a flex row it sits in and wrap there. Without it a flex item's
            // minimum width is its whole unwrapped line (GPUI measures text with no wrap width for that query), so a
            // long session title in a dialog summary ran past the dialog's edge and was cut off there, hiding the part
            // that tells two look-alike titles apart.
            .min_w_0()
            .child(text)
    }
}

/// One line of wording a component shows: the app's own words, or a [`HostSentence`] that names a host value.
///
/// Components whose text is usually the app's own but sometimes names something a host wrote (a dialog's title and
/// summary, a prompt's lines: "Delete «api-refactor» on «build-box»?") take a `Line`, so a caller never has to turn a
/// host value into a string to fit a string parameter. Without it, the only way in would be formatting
/// [`HostText::raw`] into a label, which is exactly the unescaped, unisolated path SPEC_impl.md, "Host-supplied text",
/// rules out. Plain strings convert through `From`, so app wording reads as it always did (`.summary("On this
/// machine")`).
///
/// An app line draws exactly as a string child would. A sentence draws through [`HostSentence`]'s own element: one
/// labelled node, under the id the caller gave it, named with its host parts escaped. A dialog titled with a sentence
/// ("Set up host · «build-box»") takes the sentence's accessible name as its own, so the host value reaches the
/// accessible name escaped too. Button labels stay plain strings, for the app's own words only. That is a convention,
/// not something the type enforces, since a label still takes any string; and a title still takes a `String` too, so
/// formatting [`HostText::raw`] into one would reach the screen and the accessible name unescaped: name a host value in
/// a title only through a sentence.
///
/// Short hover help is a `Line` too: a control's help that names a host value ("Claude Code on «build-box»") is built
/// with [`HoverHelp::sentence`](crate::components::HoverHelp::sentence), and the layer manager draws the tooltip from
/// the `Line` itself, so the host value is laid out by this module's element there as anywhere else. Help in
/// paragraphs is a list of sentences instead ([`HoverHelp::paragraphs`](crate::components::HoverHelp::paragraphs)).
#[derive(Clone, Debug, PartialEq, Eq, IntoElement)]
pub enum Line {
    /// The app's own words.
    App(SharedString),
    /// A sentence mixing the app's words with host values.
    Sentence(HostSentence),
}

impl Line {
    /// The line as one string, as assistive technology reads it: the app's words as they are, or a sentence's
    /// [`HostSentence::accessible_name`], with every host value escaped. For accessible names and descriptions (a
    /// control's description is its hover help's text) and for tests; never for display, where a sentence must be drawn
    /// as its own element so each host value keeps its direction to itself.
    pub fn accessible_text(&self) -> SharedString {
        match self {
            Line::App(words) => words.clone(),
            Line::Sentence(sentence) => sentence.accessible_name(),
        }
    }
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

    /// The words a painted line shows: `painted` (a line of text from the driver record) without the isolates a
    /// sentence wraps each host value in, which draw as nothing. For tests that look for words on screen; a test that
    /// checks a host value was isolated uses [`paints_isolated`] on the line as painted instead.
    ///
    /// Only the pairs a sentence adds go: an opening isolate with its closing one after it and no other isolate
    /// between. A lone or unbalanced one stays, so a check for presentation-unsafe characters still catches an isolate
    /// that reached the screen any other way.
    pub(crate) fn painted_words(painted: &str) -> String {
        let mut words = String::new();
        let mut rest = painted;
        while let Some(open) = rest.find(LRI) {
            let (before, after) = (&rest[..open], &rest[open + LRI.len_utf8()..]);
            words.push_str(before);
            match after.find([LRI, PDI]) {
                Some(close) if after[close..].starts_with(PDI) => {
                    words.push_str(&after[..close]);
                    rest = &after[close + PDI.len_utf8()..];
                }
                _ => {
                    words.push(LRI);
                    rest = after;
                }
            }
        }
        words.push_str(rest);
        words
    }

    /// Whether `painted` (a line of text from the driver record) paints the host value whose presentation is `shown`
    /// as a sentence does: escaped, inside its isolate, which also holds the spaces the app's words before it end
    /// with. A test that finds this knows the value was drawn escaped and could not reorder the words around it.
    pub(crate) fn paints_isolated(painted: &str, shown: &str) -> bool {
        painted.match_indices(LRI).any(|(at, _)| {
            let inside = &painted[at + LRI.len_utf8()..];
            inside.find([LRI, PDI]).is_some_and(|close| {
                inside[close..].starts_with(PDI)
                    && inside[..close]
                        .strip_suffix(shown)
                        .is_some_and(|spaces| spaces.chars().all(|ch| ch == ' '))
            })
        })
    }

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
    /// and its escaped accessible name as the label, without the isolates its drawn text holds; and the nodes inside it
    /// (its text element, at least) have no role of their own.
    #[gpui::test]
    fn a_drawn_sentence_is_one_labelled_node_with_its_escaped_name(cx: &mut gpui::TestAppContext) {
        crate::test_support::install(cx);
        let (_, cx) = cx.add_window_view(|_, _| SentenceView);
        let nodes = crate::test_support::frame(cx);
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
        // Every node recorded below the sentence, at any depth: its text element.
        let inside = |mut index: usize| loop {
            match nodes[index].parent {
                Some(parent) if parent == sentence => return true,
                Some(parent) => index = parent,
                None => return false,
            }
        };
        let parts: Vec<_> = (0..nodes.len()).filter(|&index| inside(index)).collect();
        assert!(
            !parts.is_empty(),
            "premise: the sentence's text element was recorded"
        );
        assert!(
            parts.iter().all(|&index| nodes[index].role.is_none()),
            "something inside the sentence has a role of its own"
        );
    }

    /// The index of the node drawn under `id` in a frame's driver record, and every line of text painted by it or by
    /// any node inside it, at any depth: a sentence's text is painted by the text element inside it, never by the
    /// sentence's own node. Panics when nothing was drawn under `id`.
    fn painted(nodes: &[gpui::DriverNode], id: &str) -> (usize, Vec<gpui::DriverText>) {
        let Some(top) = nodes
            .iter()
            .position(|node| node.element_id.as_deref() == Some(id))
        else {
            panic!("{id} was not drawn");
        };
        let inside = |mut index: usize| loop {
            if index == top {
                return true;
            }
            match nodes[index].parent {
                Some(parent) => index = parent,
                None => return false,
            }
        };
        let texts = (0..nodes.len())
            .filter(|&index| inside(index))
            .flat_map(|index| nodes[index].texts.iter().cloned())
            .collect();
        (top, texts)
    }

    /// Draws each spoofing string twice, once as a host part and once as a monospace part, in sentences `host-<n>` and
    /// `mono-<n>`.
    struct MonoCorpusView;

    impl gpui::Render for MonoCorpusView {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            div()
                .flex()
                .flex_col()
                .children(
                    SPOOFING_CORPUS
                        .iter()
                        .enumerate()
                        .flat_map(|(n, (raw, _))| {
                            let text = HostText::from_host(*raw);
                            [
                                HostSentence::new(SharedString::from(format!("host-{n}")))
                                    .text("At ")
                                    .host(&text),
                                HostSentence::new(SharedString::from(format!("mono-{n}")))
                                    .text("At ")
                                    .mono(&text),
                            ]
                        }),
                )
        }
    }

    /// Why this matters: a monospace part exists only to change the face a technical host value is drawn in. Its
    /// escaping and direction isolation come from how it is drawn (its escaped presentation inside an isolate), and its
    /// accessible name (which is also a hover help's description) from how the sentence is named; a mono part that
    /// differed from a host part in either would put a path's hidden characters on screen or in a screen reader raw.
    ///
    /// Specification: with GPUI's driver record on, for every string in the spoofing corpus, the monospace sentence is
    /// one `Label` node named like the host sentence, the app's words followed by the escaped value, and paints the
    /// same text as the host sentence: one line, the app's words and then the escaped value between the isolate's
    /// opening and closing characters.
    #[gpui::test]
    fn a_drawn_mono_part_is_escaped_and_isolated_as_a_host_part(cx: &mut gpui::TestAppContext) {
        crate::test_support::install(cx);
        let (_, cx) = cx.add_window_view(|_, _| MonoCorpusView);
        let nodes = crate::test_support::frame(cx);
        let drawn = |id: &str| {
            let (top, texts) = painted(&nodes, id);
            let lines: Vec<String> = texts.iter().map(|text| text.text.to_string()).collect();
            (&nodes[top], lines)
        };
        for (n, (_, shown)) in SPOOFING_CORPUS.iter().enumerate() {
            let (host, host_painted) = drawn(&format!("host-{n}"));
            let (mono, mono_painted) = drawn(&format!("mono-{n}"));
            assert_eq!(mono.role, Some(Role::Label), "{shown:?}");
            assert_eq!(mono.label, host.label, "{shown:?}");
            assert_eq!(mono.label.as_deref(), Some(format!("At {shown}").as_str()));
            assert_eq!(mono_painted, host_painted, "{shown:?}");
            assert_eq!(
                mono_painted,
                [format!("At{LRI} {shown}{PDI}")],
                "{shown:?} was not painted escaped inside its isolate"
            );
        }
    }

    /// Why this matters: the monospace face is the part's whole reason to exist (paths and versions line up and read
    /// as technical values, as in the old app), and GPUI's test text system gives every character the same width in
    /// any face, so only the real text system with the bundled fonts can tell which face a part was drawn in.
    ///
    /// Specification: laid out with the real text system, a monospace part of eight `i`s and one of eight `M`s are
    /// both eight advances of the terminal face wide, while the same values as host parts differ in width (the
    /// premise: the interface face is proportional, so the comparison can tell the faces apart). The isolate around
    /// each value is inside the measured line, so this also pins that it is drawn as nothing.
    #[test]
    fn a_mono_part_is_drawn_in_the_monospace_face() {
        let (mut cx, _, nodes) = draw_real(linux_text_system(), || WidthView);
        let width = |id: &str| {
            let (_, texts) = painted(&nodes, id);
            assert_eq!(texts.len(), 1, "{id} paints one line: {texts:?}");
            f32::from(texts[0].bounds.size.width)
        };
        let advance = cx.update(|cx| {
            let text_system = cx.text_system();
            let face = text_system.resolve_font(&gpui::font(assets::TERMINAL_FONT));
            f32::from(text_system.advance(face, gpui::px(13.), 'M').unwrap().width)
        });
        let (host_narrow, host_wide) = (width("host-narrow"), width("host-wide"));
        assert!(
            host_wide - host_narrow > 10.,
            "premise: the interface face is proportional ({host_narrow} vs {host_wide})"
        );
        for id in ["mono-narrow", "mono-wide"] {
            let drawn = width(id);
            assert!(
                (drawn - 8. * advance).abs() < 0.5,
                "{id} is {drawn} wide, not eight terminal-face advances ({advance} each)"
            );
        }
    }

    /// Draws two values, one narrow and one wide in a proportional face, as host parts and as monospace parts, one
    /// sentence each, at 13 pixels in the interface face.
    struct WidthView;

    impl gpui::Render for WidthView {
        fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let narrow = HostText::from_host("iiiiiiii");
            let wide = HostText::from_host("MMMMMMMM");
            div()
                .flex()
                .flex_col()
                .items_start()
                .font_family(assets::UI_FONT)
                .text_size(gpui::px(13.))
                .child(HostSentence::new("host-narrow").host(&narrow))
                .child(HostSentence::new("host-wide").host(&wide))
                .child(HostSentence::new("mono-narrow").mono(&narrow))
                .child(HostSentence::new("mono-wide").mono(&wide))
        }
    }

    // ---- Flow and isolation, on real text systems -------------------------------------------------------------------
    //
    // A sentence's guarantees rest on how a real shaper treats its one text element: where it breaks lines, how it
    // orders right-to-left letters, and whether the isolates draw as anything. The checks below take the text system
    // as a parameter, because the platforms shape differently: the Linux engine (cosmic-text), which the goldens use,
    // and on macOS the platform's own (CoreText, through GPUI's Mac text system), which is what a Mac user's text goes
    // through and which the Linux engine cannot stand in for.

    /// The size and line height the flow views draw at, so a test can shape the same text itself.
    const FLOW_SIZE: f32 = 13.;
    const FLOW_LINE: f32 = 18.;

    /// The Linux engine with only the bundled fonts, as the goldens and `test_support::headless` set it up.
    fn linux_text_system() -> Arc<dyn gpui::PlatformTextSystem> {
        Arc::new(gpui_wgpu::CosmicTextSystem::new_without_system_fonts(
            assets::UI_FONT,
        ))
    }

    /// One run of `len` bytes in the interface face, the run a sentence with no monospace part is drawn as.
    fn ui_run(len: usize) -> gpui::TextRun {
        gpui::TextStyle {
            font_family: assets::UI_FONT.into(),
            ..gpui::TextStyle::default()
        }
        .to_run(len)
    }

    /// Opens `view` in a headless window over `text_system`, bootstrapped like the app (the bundled fonts installed),
    /// draws one frame with the driver record on, and returns the context, the window and the record. No renderer:
    /// nothing here is captured, so it runs wherever the text system does.
    fn draw_real<V: gpui::Render>(
        text_system: Arc<dyn gpui::PlatformTextSystem>,
        view: impl FnOnce() -> V,
    ) -> (
        gpui::HeadlessAppContext,
        gpui::AnyWindowHandle,
        Vec<gpui::DriverNode>,
    ) {
        use gpui::{AppContext as _, px, size};

        let mut cx =
            gpui::HeadlessAppContext::with_asset_source(text_system, Arc::new(assets::Assets));
        cx.update(crate::bootstrap::install).unwrap();
        let window = cx
            .open_window(size(px(800.), px(400.)), |_, cx| cx.new(|_| view()))
            .unwrap()
            .into();
        let nodes = cx
            .update_window(window, |_, window, cx| {
                window.set_driver_enabled(true);
                window.draw(cx).clear(cx);
                window.driver_nodes().to_vec()
            })
            .unwrap();
        (cx, window, nodes)
    }

    /// `text` shaped in `window` in the interface face, as a sentence with no monospace part is, wrapped at `width`
    /// when given; the byte index each wrapped line after the first starts at.
    fn wrap_breaks(
        cx: &mut gpui::HeadlessAppContext,
        window: gpui::AnyWindowHandle,
        text: &SharedString,
        width: gpui::Pixels,
    ) -> Vec<usize> {
        let shaped = cx
            .update_window(window, |_, window, _| {
                window
                    .text_system()
                    .shape_text(
                        text.clone(),
                        gpui::px(FLOW_SIZE),
                        &[ui_run(text.len())],
                        Some(width),
                        None,
                    )
                    .unwrap()
            })
            .unwrap();
        assert_eq!(shaped.len(), 1, "premise: {text:?} holds no hard break");
        let line = &shaped[0];
        line.wrap_boundaries()
            .iter()
            .map(|at| line.unwrapped_layout.runs[at.run_ix].glyphs[at.glyph_ix].index)
            .collect()
    }

    /// `text` shaped on one line in `window` in the interface face.
    fn shape(
        cx: &mut gpui::HeadlessAppContext,
        window: gpui::AnyWindowHandle,
        text: &str,
    ) -> gpui::ShapedLine {
        cx.update_window(window, |_, window, _| {
            window.text_system().shape_line(
                SharedString::from(text.to_string()),
                gpui::px(FLOW_SIZE),
                &[ui_run(text.len())],
                None,
            )
        })
        .unwrap()
    }

    /// The YOLO question's sentence, with its host value mid-sentence: the sentence the launcher reported breaking
    /// early in the sidebar row.
    fn mid_sentence_host(id: &'static str) -> HostSentence {
        HostSentence::new(id)
            .text("Farhelm stops its actions on ")
            .host(&HostText::from_host("build-box"))
            .text(" without asking you.")
    }

    /// The words of [`mid_sentence_host`] that [`FlowView`]'s width is fitted to: the value with app words on both
    /// sides, so a sentence that flows must keep the value on a line with app words.
    const FLOW_FIT: &str = "its actions on build-box without";

    /// Draws [`mid_sentence_host`] as `flow`, in a box just wide enough for [`FLOW_FIT`] and far too narrow for the
    /// whole sentence.
    struct FlowView;

    impl gpui::Render for FlowView {
        fn render(&mut self, window: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
            let fit = window
                .text_system()
                .shape_line(
                    FLOW_FIT.into(),
                    gpui::px(FLOW_SIZE),
                    &[ui_run(FLOW_FIT.len())],
                    None,
                )
                .width;
            div()
                .font_family(assets::UI_FONT)
                .text_size(gpui::px(FLOW_SIZE))
                .line_height(gpui::px(FLOW_LINE))
                .child(div().w(fit + gpui::px(2.)).child(mid_sentence_host("flow")))
        }
    }

    /// The flow check, on the frame [`FlowView`] drew: the sentence paints one text, its drawn string, as tall as that
    /// string's own wrapped lines at the box's width; every break follows a space, so no wrapped line starts with one;
    /// and the line holding the host value holds app words too.
    fn check_flow(
        cx: &mut gpui::HeadlessAppContext,
        window: gpui::AnyWindowHandle,
        nodes: &[gpui::DriverNode],
    ) {
        let (_, texts) = painted(nodes, "flow");
        assert_eq!(
            texts.len(),
            1,
            "the sentence paints one text, not one per part: {texts:?}"
        );
        let drawn = &texts[0];
        assert_eq!(drawn.text, mid_sentence_host("flow").drawn().0);
        let breaks = wrap_breaks(cx, window, &drawn.text, drawn.bounds.size.width);
        assert!(
            !breaks.is_empty(),
            "premise: the box is too narrow for one line"
        );
        assert_eq!(
            f32::from(drawn.bounds.size.height),
            (breaks.len() + 1) as f32 * FLOW_LINE,
            "the sentence is drawn as tall as its own words wrapped at its width"
        );
        let text = drawn.text.as_ref();
        let mut lines = Vec::new();
        let mut seen = 0;
        for at in breaks {
            lines.push(&text[seen..at]);
            seen = at;
        }
        lines.push(&text[seen..]);
        for wrapped in &lines[..lines.len() - 1] {
            assert!(wrapped.ends_with(' '), "a line breaks mid-word: {lines:?}");
        }
        let holding = lines
            .iter()
            .find(|wrapped| wrapped.contains("build-box"))
            .expect("the value was drawn whole on one line");
        assert!(
            painted_words(holding)
                .replace("build-box", "")
                .chars()
                .any(char::is_alphabetic),
            "the value sits on a line of its own: {lines:?}"
        );
    }

    /// The break-before-a-value check: [`mid_sentence_host`] wrapped just wider than the words before its value
    /// breaks right before the value, so the first line is exactly those words. GPUI keeps a break opportunity only
    /// where a glyph starts, and on macOS an isolate opened right after the space lost this break and wrapped "on"
    /// onto the value's line ([`HostSentence::drawn`] says how the break is kept).
    fn check_break_before_value(cx: &mut gpui::HeadlessAppContext, window: gpui::AnyWindowHandle) {
        const BEFORE: &str = "Farhelm stops its actions on ";
        let (text, _) = mid_sentence_host("flow").drawn();
        let width = shape(cx, window, BEFORE).width + gpui::px(2.);
        let breaks = wrap_breaks(cx, window, &text, width);
        // The isolate opens before the space, so the first line may end with it; it draws as nothing.
        let first = breaks.first().map(|&at| text[..at].replace(LRI, ""));
        assert_eq!(
            first.as_deref(),
            Some(BEFORE),
            "the line breaks right before the value: {text:?} at {breaks:?}"
        );
    }

    /// The isolates-draw-nothing check: an all-left-to-right sentence is as wide drawn as its words alone (within half a
    /// pixel, for kerning the isolate may interrupt), and every glyph the shaper made for an isolate rasterizes to
    /// nothing, so no box or other mark shows at a value's edges. The width comparison carries the check on both
    /// platforms: CoreText makes no glyph at an isolate and the Linux shaper turns it into a zero-width space, so the
    /// raster loop only guards against a future shaper drawing a visible mark there.
    fn check_isolates_draw_nothing(
        cx: &mut gpui::HeadlessAppContext,
        window: gpui::AnyWindowHandle,
        text_system: &dyn gpui::PlatformTextSystem,
    ) {
        let (text, _) = mid_sentence_host("flow").drawn();
        let drawn = shape(cx, window, &text);
        let words = shape(cx, window, &painted_words(&text));
        assert!(
            (f32::from(drawn.width) - f32::from(words.width)).abs() < 0.5,
            "the isolates take no width: {:?} drawn, {:?} without them",
            drawn.width,
            words.width
        );
        let isolates: Vec<usize> = text
            .char_indices()
            .filter(|(_, ch)| matches!(*ch, LRI | PDI))
            .map(|(at, _)| at)
            .collect();
        for run in &drawn.runs {
            for glyph in run
                .glyphs
                .iter()
                .filter(|glyph| isolates.contains(&glyph.index))
            {
                let bounds = text_system
                    .glyph_raster_bounds(&gpui::RenderGlyphParams {
                        font_id: run.font_id,
                        glyph_id: glyph.id,
                        font_size: gpui::px(FLOW_SIZE),
                        subpixel_variant: gpui::Point::default(),
                        scale_factor: 2.,
                        is_emoji: glyph.is_emoji,
                        subpixel_rendering: false,
                        dilation: 0,
                    })
                    .unwrap();
                assert!(
                    bounds.size.width.0 == 0 || bounds.size.height.0 == 0,
                    "the isolate at byte {} draws a glyph {bounds:?}",
                    glyph.index
                );
            }
        }
    }

    /// Two right-to-left host values, three Hebrew letters each, with letters of their own so a check can tell where
    /// each was drawn.
    const ALEF: &str = "\u{5d0}\u{5d1}\u{5d2}";
    const DALET: &str = "\u{5d3}\u{5d4}\u{5d5}";

    /// Whether every glyph of `left` (its first occurrence) is drawn left of every glyph of `right`, with `text`
    /// shaped on one line in `window`.
    fn drawn_left_of(
        cx: &mut gpui::HeadlessAppContext,
        window: gpui::AnyWindowHandle,
        text: &str,
        left: &str,
        right: &str,
    ) -> bool {
        let line = shape(cx, window, text);
        let xs = |needle: &str| -> Vec<f32> {
            let at = text.find(needle).unwrap();
            line.runs
                .iter()
                .flat_map(|run| &run.glyphs)
                .filter(|glyph| (at..at + needle.len()).contains(&glyph.index))
                .map(|glyph| f32::from(glyph.position.x))
                .collect()
        };
        let (left, right) = (xs(left), xs(right));
        assert!(
            !left.is_empty() && !right.is_empty(),
            "nothing shaped in {text:?}"
        );
        left.iter().all(|l| right.iter().all(|r| l < r))
    }

    /// The isolation check, on drawn strings shaped in `window`. Three sentences, each with a premise that the threat
    /// is real on this shaper, checked first:
    ///
    /// - "At «אבג», «דהו».": the first value is drawn left of the second. Premise: in one plain string with no
    ///   isolates, the second is drawn left of the first.
    /// - "«אבג» asks first.": the value is drawn left of the app's words. Premise: in a plain string, a leading value
    ///   turns the line right-to-left and is drawn right of them.
    /// - "At «אבג abc».": inside the value, `abc` is drawn right of the Hebrew, which only a left-to-right isolate
    ///   gives, as the old app's `dir="ltr"` spans did. Premise: inside a right-to-left isolate, `abc` is drawn left.
    fn check_isolation(cx: &mut gpui::HeadlessAppContext, window: gpui::AnyWindowHandle) {
        let (alef, dalet) = (HostText::from_host(ALEF), HostText::from_host(DALET));
        let mixed = format!("{ALEF} abc");
        let pair = HostSentence::new("pair")
            .text("At ")
            .host(&alef)
            .text(", ")
            .host(&dalet)
            .text(".");
        let lead = HostSentence::new("lead").host(&alef).text(" asks first.");
        let inner = HostSentence::new("inner")
            .text("At ")
            .host(&HostText::from_host(mixed.as_str()))
            .text(".");
        for (sentence, plain, first, second) in [
            (pair, format!("At {ALEF}, {DALET}."), ALEF, DALET),
            (lead, format!("{ALEF} asks first."), ALEF, "asks"),
            (inner, format!("At \u{2067}{mixed}{PDI}."), ALEF, "abc"),
        ] {
            assert!(
                drawn_left_of(cx, window, &plain, second, first),
                "premise: the shaper reorders {plain:?}"
            );
            let (drawn, _) = sentence.drawn();
            assert!(
                drawn_left_of(cx, window, &drawn, first, second),
                "{first:?} is drawn left of {second:?} in {drawn:?}"
            );
        }
    }

    /// Why this matters: a sentence laid out as one element per part could only wrap whole parts, so in a narrow place
    /// (the sidebar row's question, a dialog summary) it broke early at the host value and started the next line on
    /// the space after it. The old app's inline text broke between words, and that is the behavior being ported.
    ///
    /// Specification: laid out with the Linux engine, [`check_flow`] and [`check_break_before_value`] hold.
    #[test]
    fn a_sentence_wraps_between_words_not_at_its_parts() {
        let (mut cx, window, nodes) = draw_real(linux_text_system(), || FlowView);
        check_flow(&mut cx, window, &nodes);
        check_break_before_value(&mut cx, window);
    }

    /// Why this matters: a sentence is one text element, and GPUI runs the bidirectional algorithm over a whole text
    /// element, across its runs, so the escape table alone does not keep a right-to-left host value from reordering
    /// the app's words around it. The isolates do, and they must draw as nothing.
    ///
    /// Specification: with the Linux engine, [`check_isolation`] and [`check_isolates_draw_nothing`] hold.
    #[test]
    fn a_right_to_left_host_part_reorders_only_within_its_isolate() {
        let text_system = linux_text_system();
        let (mut cx, window, _) = draw_real(text_system.clone(), || FlowView);
        check_isolation(&mut cx, window);
        check_isolates_draw_nothing(&mut cx, window, &*text_system);
    }

    /// Why this matters: on macOS GPUI shapes a whole line as one CoreText line, so a sentence keeps right-to-left
    /// values to themselves only if CoreText honors the isolates, draws them as nothing, and leaves GPUI a glyph to
    /// break before. The Linux engine's tests prove none of that for a Mac, where the maintainer runs Farhelm.
    ///
    /// Specification: with macOS's own text system and the bundled fonts, [`check_flow`],
    /// [`check_break_before_value`], [`check_isolation`] and [`check_isolates_draw_nothing`] hold.
    #[cfg(target_os = "macos")]
    #[test]
    fn on_macos_core_text_keeps_a_sentence_flowing_and_its_host_parts_isolated() {
        let text_system: Arc<dyn gpui::PlatformTextSystem> =
            Arc::new(gpui_apple::AppleTextSystem::new());
        let (mut cx, window, nodes) = draw_real(text_system.clone(), || FlowView);
        check_flow(&mut cx, window, &nodes);
        check_break_before_value(&mut cx, window);
        check_isolation(&mut cx, window);
        check_isolates_draw_nothing(&mut cx, window, &*text_system);
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
        let nodes = crate::test_support::frame(cx);
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

    /// Why this matters: an identity is shown so the user can tell two of them apart (an install that now reports a
    /// different id, the session that asked for an approval), which only works if no two identities look alike and the
    /// ordinary ones (UUIDs, domain names, addresses) still read as themselves.
    ///
    /// Specification: printable ASCII other than `<`, `(` and `)` is shown as itself; each of those three, a space, a
    /// non-ASCII letter, a variation selector and a control character is shown as `<U+XXXX>`; the empty identity is
    /// `(empty)`, which a host's literal `(empty)` cannot imitate; and the raw value keeps the host's bytes.
    #[test]
    fn an_identity_spells_everything_but_plain_ascii() {
        for (raw, shown) in [
            (
                "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d",
                "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d",
            ),
            ("build-1.example.com:22", "build-1.example.com:22"),
            ("!~", "!~"),
            ("<U+0020>", "<U+003C>U+0020>"),
            ("a(b)", "a<U+0028>b<U+0029>"),
            ("a b", "a<U+0020>b"),
            ("caf\u{e9}", "caf<U+00E9>"),
            ("\u{2764}\u{FE0F}", "<U+2764><U+FE0F>"),
            ("a\u{1b}", "a<U+001B>"),
            ("", "(empty)"),
            ("(empty)", "<U+0028>empty<U+0029>"),
        ] {
            let text = HostText::identity(raw);
            assert_eq!(text.presentation().as_ref(), shown, "identity {raw:?}");
            assert_eq!(text.raw(), raw);
        }
    }

    /// Why this matters: the approval card's From line has room for only the ends of the asking session's id (SPEC.md,
    /// "Agent-spawned sessions"), and a cut through an escape would show something that is the literal spelling of a
    /// different id. Ported from the old app's
    /// `requester_abbreviation_preserves_identity_tokens_and_bare_suffix`.
    ///
    /// Specification: one entry prefix is dropped and the bare id is shown in the identity spelling, whole when 16
    /// shown characters or fewer, otherwise as about its first 8 and last 4 around `…` with escapes at a cut kept whole;
    /// a value whose kept ends meet is shown whole, with no `…`; a bare id that looks like a full reference keeps its
    /// look-alike prefix; a value that is not a reference is shortened whole. The raw value stays the whole reference,
    /// and a sentence holding the part, like `Debug`, names it by the identity spelling of the whole reference.
    #[test]
    fn an_abbreviated_identity_keeps_escapes_whole_and_the_bare_suffix() {
        const ENTRY: &str = "7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d";
        let reference = |bare: &str| {
            farhelm_proto::SessionRef::new(ENTRY, bare)
                .unwrap()
                .to_string()
        };
        let nested = reference("x");
        for (raw, shown) in [
            (reference("short"), "short"),
            // The cutoff: 16 shown characters are kept whole, 17 are shortened.
            (reference("1234567890abcdef"), "1234567890abcdef"),
            (reference("1234567890abcdefg"), "12345678…defg"),
            (reference("1234567890abcdefghijkl"), "12345678…ijkl"),
            (
                reference("1234567\u{200b}middle\u{202e}"),
                "1234567<U+200B>…<U+202E>",
            ),
            (reference("<U+200B>middle-long-enough"), "<U+003C>…ough"),
            // Two escapes and a letter: past 16 shown characters, but the head and tail meet with nothing between them,
            // so it is shown whole rather than with an ellipsis that hides nothing.
            (reference("\u{200b}\u{200c}a"), "<U+200B><U+200C>a"),
            (reference(&nested), "7f8c2b1e…6d:x"),
            ("1234567890abcdefghijkl".to_string(), "12345678…ijkl"),
        ] {
            let text = HostText::identity_abbreviated(raw.as_str());
            assert_eq!(
                text.presentation().as_ref(),
                shown,
                "abbreviation of {raw:?}"
            );
            assert_eq!(text.raw(), raw, "the raw value keeps the entry prefix");
        }
        let id = HostText::identity_abbreviated(reference("1234567\u{200b}middle\u{202e}"));
        assert_eq!(
            HostSentence::new("from")
                .text("ID ")
                .mono(&id)
                .accessible_name()
                .as_ref(),
            "ID 7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:1234567<U+200B>middle<U+202E>"
        );
        assert_eq!(
            format!("{id:?}"),
            "HostText(\"7f8c2b1e-6d0e-4c51-9a8e-1e2f3a4b5c6d:1234567<U+200B>middle<U+202E>\")"
        );
    }

    /// Why this matters: equality decides whether something drawn must be redrawn or refreshed (the layer manager
    /// refreshes a hovered control's kept help only when the new help is unequal), so two spellings of one raw value
    /// that draw differently, or are named differently, must not compare equal, or the stale one stays.
    ///
    /// Specification: one raw value taken as ordinary text, as an identity and as an abbreviated identity, where each
    /// draws differently, gives three unequal values, which still hash equal (hashing uses the raw value alone); a
    /// value that an abbreviation draws exactly as ordinary text does but names differently (`12345678…ijkl`, whose
    /// `…` the identity spelling escapes) is unequal to it; values that draw and are named alike (an identity short
    /// enough to be shown whole, and its abbreviation) are equal.
    #[test]
    fn spellings_of_one_value_that_differ_are_different_values() {
        use std::hash::BuildHasher as _;

        let raw = "1234567890 abcdefghijkl";
        let plain = HostText::from_host(raw);
        let identity = HostText::identity(raw);
        let abbreviated = HostText::identity_abbreviated(raw);
        assert_eq!(plain.presentation().as_ref(), raw, "premise");
        assert_eq!(
            identity.presentation().as_ref(),
            "1234567890<U+0020>abcdefghijkl",
            "premise"
        );
        assert_eq!(
            abbreviated.presentation().as_ref(),
            "12345678…ijkl",
            "premise"
        );
        assert_ne!(plain, identity);
        assert_ne!(plain, abbreviated);
        assert_ne!(identity, abbreviated);
        let hasher = std::collections::hash_map::RandomState::new();
        assert_eq!(hasher.hash_one(&plain), hasher.hash_one(&identity));
        assert_eq!(hasher.hash_one(&identity), hasher.hash_one(&abbreviated));

        let drawn_alike = "12345678…ijkl";
        let plain = HostText::from_host(drawn_alike);
        let abbreviated = HostText::identity_abbreviated(drawn_alike);
        assert_eq!(plain.presentation(), abbreviated.presentation(), "premise");
        assert_eq!(
            abbreviated.accessible().as_ref(),
            "12345678<U+2026>ijkl",
            "premise"
        );
        assert_ne!(plain, abbreviated);

        let short = HostText::identity("a b");
        let short_abbreviated = HostText::identity_abbreviated("a b");
        assert_eq!(short, short_abbreviated);
    }

    /// Why this matters: equality decides whether a pushed change touched a value (a draft, a selection), and two host
    /// values that differ only by a hidden character are exactly the ones that must not be treated as the same.
    ///
    /// Specification: a value and its look-alike with a zero-width space are different values, and a value equals
    /// itself. Equality compares the raw bytes and both spellings (the `PartialEq` impl), and this test cannot tell
    /// which of them decided: escaping is injective (every `<U+` in a presentation is an escape, so it decodes back to
    /// one raw value), so no pair of ordinary values differs in one and not the other. It pins the outcome, not the
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

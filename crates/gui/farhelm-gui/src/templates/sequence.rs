//! Quick-launch sequences: what an armed Cmd+X (Ctrl+Shift+X on Linux) sequence holds, how each key changes it, what
//! its pill shows, and what Enter resolves it to. No GPUI types.
//!
//! SPEC.md, "Creation", "Quick-launch sequences" is the authority; SPEC_impl.md, "Launch templates", adds the
//! resolution order and the captured bindings. Ported from the old app's `launch_sequence.rs` (`SequenceState`,
//! `matching_template_names`, `match_typed_word`, `capture_sequence_bindings` and the two resolvers), the sequence
//! refusals in `list/view.rs`, and the armed branch of `assets/template-shortcuts.js` at reference commit `24bc0580`.
//! The old app split this state between a JS keyboard listener and Rust, and needed hidden relays, press snapshots and
//! generation fences to keep the two in step; the GPUI app takes keys in order on one thread, so none of that survives
//! here.
//!
//! Here's the TLDR of the model:
//!
//! - A [`Sequence`] starts when the prefix is accepted, with its own copy of the ⚡ mark and the number keys (SPEC.md:
//!   "Bindings refresh when the prefix starts; Enter captures the bound names currently shown in the pill"). Because
//!   the copy is the sequence's own, the names the pill shows and the names Enter resolves are the same by
//!   construction: a key moved elsewhere while the sequence is armed cannot change what Enter launches. The bindings
//!   are the only thing it copies: template names are read from the store each time they are needed (Tab, the
//!   candidates, Enter) and passed in, so there is no second list of templates to go stale.
//! - [`Sequence::input`] applies one [`Input`]: typed characters build the unfinished word, Space and chords queue
//!   [`Entry`]s, and Backspace edits only the unfinished word. [`Sequence::tab`] completes the word against the names.
//! - [`Sequence::view`] is what the pill draws: one [`Chip`] per queued entry, the word in progress and at most
//!   [`CANDIDATE_LIMIT`] candidates.
//! - [`Sequence::enter`] ends the sequence and resolves it, all or nothing, against the template names current at
//!   Enter, giving an [`Outcome`]: the names to apply in order, a [`Refusal`], or nothing.
//!
//! Not here: turning GPUI key events into [`Input`] (which keys cancel, the modifier rules, held repeats, the numeric
//! keypad), the cancel conditions (Escape, a click, a dialog), drawing, and applying or launching the result. Names are
//! raw strings throughout; whatever draws them escapes them as host text.

use super::Hint;
use super::shortcuts::Bindings;
use super::summary::Part;

/// The most candidates the list under the pill shows (SPEC.md: "at most five matching template names"). It caps only
/// the display: a word is ambiguous by every name that matches it, shown or not.
pub const CANDIDATE_LIMIT: usize = 5;

// ---------------------------------------------------------------------------------------------------------------------
// Keys and the queue
// ---------------------------------------------------------------------------------------------------------------------

/// One key the GPUI layer has already classified as belonging to the armed sequence. Tab and Enter are not inputs:
/// both need the store's template names, so they are [`Sequence::tab`] and [`Sequence::enter`].
///
/// A number key's identity is physical (SPEC.md: "Matching uses the physical digit key, independent of the printed
/// character"), so the number variants carry the digit as [`super::shortcuts::KEYS`] spells it, which is also how the
/// bindings map spells it; only the printed character extends a word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    /// A printable character, Shift capitals included, as [`printable`] admits it.
    Character(char),
    /// A bare physical number key: `key` is its digit, `text` the character it prints on this keyboard layout. It
    /// extends a word already in progress with `text`, and with no word queues the number key `key`. The GPUI layer
    /// sends this only when the printed text passes [`printable`]; a digit key that prints nothing typeable (a dead key)
    /// is the caller's to judge, like any other unprintable key.
    Digit { key: String, text: char },
    /// A number chord (Cmd+digit, or Ctrl+Shift+digit on Linux) for the digit `key`.
    NumberChord(String),
    /// The ⚡ chord.
    MarkChord,
    /// Space, with or without Shift.
    Space,
    /// Backspace.
    Backspace,
}

/// The character a key's printed text types into the word, or `None` when it is not one printable character: a dead
/// key's name, a control character, or several scalars. The armed listener's rule in the old app, which counted Unicode
/// scalars rather than UTF-16 units so an astral character such as an emoji is still one character. A key with no
/// printable character cancels the sequence, which is the caller's to do. Space passes this rule too, so the caller
/// classifies Space as [`Input::Space`] before asking.
pub fn printable(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        // `char::is_control` is exactly the C0 and C1 ranges (U+0000–U+001F, U+007F–U+009F) the old listener refused.
        (Some(c), None) if !c.is_control() => Some(c),
        _ => None,
    }
}

/// One queued reference, in the order the user queued it. Repetition counts: queuing the same template twice applies it
/// twice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Entry {
    /// A number key, by digit, resolved through the sequence's copy of the bindings.
    Number(String),
    /// The ⚡ mark, resolved through the sequence's copy of the mark.
    Mark,
    /// A word as typed, queued by Space, a chord or Enter, or the full name Tab completed it to. The two are one entry
    /// because Enter resolves both by the same matching rules (SPEC_impl.md: "Typed references, including Tab's
    /// completed spelling, still use the matching rules at Enter"), so a completed name is not an exact-name reference;
    /// the old app queued both the same way.
    Word(String),
}

/// The state of one armed sequence, from the accepted prefix to Enter or cancellation. `None` in the caller means idle;
/// a fresh `Sequence` with nothing queued is an armed sequence the user has not typed into yet.
#[derive(Clone, Debug)]
pub struct Sequence {
    entries: Vec<Entry>,
    word: String,
    bindings: Bindings,
}

impl Sequence {
    /// Arm a sequence with `bindings`, the ⚡ mark and number keys as the store holds them when the prefix is accepted.
    pub fn new(bindings: Bindings) -> Self {
        Sequence {
            entries: Vec::new(),
            word: String::new(),
            bindings,
        }
    }

    /// The queued entries, in order.
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// The unfinished word, empty when none is in progress.
    pub fn word(&self) -> &str {
        &self.word
    }

    /// Apply one key (SPEC.md, "Quick-launch sequences"). A chord queues any unfinished word before its own reference,
    /// so the queue keeps the order the user typed. Backspace removes one Unicode scalar from the unfinished word and
    /// never a queued entry.
    pub fn input(&mut self, input: Input) {
        match input {
            Input::Character(c) => self.word.push(c),
            Input::Digit { key, text } => {
                // SPEC.md: "a name beginning with a digit is reachable only by its number key or the launcher".
                if self.word.is_empty() {
                    self.entries.push(Entry::Number(key));
                } else {
                    self.word.push(text);
                }
            }
            Input::NumberChord(key) => {
                self.queue_word();
                self.entries.push(Entry::Number(key));
            }
            Input::MarkChord => {
                self.queue_word();
                self.entries.push(Entry::Mark);
            }
            Input::Space => self.queue_word(),
            Input::Backspace => {
                self.word.pop();
            }
        }
    }

    /// Tab: queue the full name when the unfinished word identifies one template in `names` (the store's template names
    /// in catalog order, or `None` while the store has none to offer), by the same precedence Enter uses, and otherwise
    /// do nothing. Completion is advisory: a Tab pressed while the names were unknown did nothing, and names arriving
    /// later do not replay it.
    pub fn tab<S: AsRef<str>>(&mut self, names: Option<&[S]>) {
        if let Some(names) = names
            && let Ok(name) = match_word(&self.word, names)
        {
            self.entries.push(Entry::Word(name.to_owned()));
            self.word.clear();
        }
    }

    /// What the pill shows now: each queued entry as a chip, through the sequence's copy of the bindings, the word in
    /// progress, and its candidates among `names` (the store's template names in catalog order, or `None` while the
    /// store has none to offer).
    pub fn view<'a, S: AsRef<str>>(&'a self, names: Option<&'a [S]>) -> View<'a> {
        let chips = self
            .entries
            .iter()
            .map(|entry| match entry {
                Entry::Word(text) => Chip::Name(text),
                Entry::Number(key) => match self.bindings.keys.get(key) {
                    Some(name) => Chip::Name(name),
                    None => Chip::Unbound(format!("Unbound key {key}")),
                },
                Entry::Mark => match &self.bindings.marked {
                    Some(name) => Chip::Name(name),
                    None => Chip::Unbound("Unbound ⚡".to_owned()),
                },
            })
            .collect();
        let candidates = match names {
            Some(names) => {
                let mut candidates = matching_names(&self.word, names);
                candidates.truncate(CANDIDATE_LIMIT);
                candidates
            }
            None => Vec::new(),
        };
        View {
            chips,
            word: &self.word,
            candidates,
        }
    }

    /// End the sequence and resolve it against `templates`, the template names current at Enter, in catalog order.
    ///
    /// The unfinished word is queued first. Resolution is all or nothing, and its order decides which problem a mixed
    /// sequence reports: every typed word is resolved first, so an unknown or ambiguous word wins over a key anywhere
    /// in the sequence (SPEC.md); then the number keys and ⚡ are read from the sequence's own copy of the bindings,
    /// where a bound name that is no longer a template refuses the sequence rather than letting anything else stand in
    /// for it, as the old dispatcher did before it fell back to the binding hint; then a reference shown as unbound
    /// refuses it, even if a binding has appeared since. Within each step the first problem in queue order is the one
    /// reported.
    pub fn enter<S: AsRef<str>>(mut self, templates: &[S]) -> Outcome {
        self.queue_word();
        if self.entries.is_empty() {
            return Outcome::Nothing;
        }
        let alone = self.entries.len() == 1;
        let mut names = Vec::with_capacity(self.entries.len());
        for entry in &self.entries {
            if let Entry::Word(word) = entry {
                match match_word(word, templates) {
                    Ok(name) => names.push(Some(name.to_owned())),
                    Err(refusal) => return Outcome::Refused(refusal),
                }
            } else {
                names.push(None);
            }
        }
        let mut unbound = None;
        for (entry, name) in self.entries.iter().zip(&mut names) {
            let (bound, reference) = match entry {
                Entry::Number(key) => (self.bindings.keys.get(key), Reference::Key(key.clone())),
                Entry::Mark => (self.bindings.marked.as_ref(), Reference::Mark),
                Entry::Word(_) => continue,
            };
            match bound {
                // Exact names only: a binding names one template, never a case variant of it.
                Some(bound) if templates.iter().any(|t| t.as_ref() == bound) => {
                    *name = Some(bound.clone());
                }
                Some(bound) => return Outcome::Refused(Refusal::Missing(bound.clone())),
                None => {
                    unbound.get_or_insert(Binding { reference, alone });
                }
            }
        }
        if let Some(binding) = unbound {
            return Outcome::Refused(Refusal::Unbound(binding));
        }
        Outcome::Apply(names.into_iter().flatten().collect())
    }

    /// Queue the unfinished word, if there is one, exactly as typed: it is matched only at Enter.
    fn queue_word(&mut self) {
        if !self.word.is_empty() {
            self.entries
                .push(Entry::Word(std::mem::take(&mut self.word)));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// What the pill shows
// ---------------------------------------------------------------------------------------------------------------------

/// What the pill draws, borrowed from the sequence and the names passed in. Names and the word are raw; the drawer
/// escapes them as host text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View<'a> {
    /// One chip per queued entry, in order.
    pub chips: Vec<Chip<'a>>,
    /// The word in progress, empty when none is.
    pub word: &'a str,
    /// The template names matching the word, exact matches before prefix matches and catalog order within each, at
    /// most [`CANDIDATE_LIMIT`]. Empty with no word in progress, no match, or no names yet.
    pub candidates: Vec<&'a str>,
}

/// One queued entry as the pill shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chip<'a> {
    /// A typed or completed word, or the template a number key or ⚡ is bound to in the sequence's copy. Raw text, to be
    /// drawn as host text.
    Name(&'a str),
    /// A number key or ⚡ with nothing bound, in the app's own words: "Unbound key 3", "Unbound ⚡".
    Unbound(String),
}

// ---------------------------------------------------------------------------------------------------------------------
// What Enter resolves to
// ---------------------------------------------------------------------------------------------------------------------

/// What Enter resolved a sequence to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing was queued: SPEC.md, "Enter with nothing queued cancels without opening anything".
    Nothing,
    /// Apply these templates, by name, in this order, later ones winning (SPEC.md). Never empty; duplicates are kept,
    /// because queuing a template twice applies it twice. Every name was in the slice given to [`Sequence::enter`].
    Apply(Vec<String>),
    /// The sequence is refused as a whole; nothing in it applies.
    Refused(Refusal),
}

/// Why a sequence was refused. The fields hold raw names and words.
///
/// Where it is shown depends on where the sequence was typed (SPEC.md). Outside the launcher, [`Refusal::hint`] says
/// whether it opens Templates with a binding hint, and otherwise New opens showing [`Refusal::sentence`]. Inside the
/// launcher nothing opens and [`Refusal::launcher_sentence`] is shown beneath search.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// A typed or completed word matched no template.
    UnknownWord(String),
    /// A typed word matched more than one template in its winning tier. `candidates` is every competitor in that tier,
    /// in catalog order, however many the candidate list had room for.
    AmbiguousWord {
        word: String,
        candidates: Vec<String>,
    },
    /// A number key or ⚡ was bound to this name in the sequence's copy, and no template has that name any more (it was
    /// renamed or deleted after the prefix).
    Missing(String),
    /// A number key or ⚡ was shown unbound: the first such reference in the queue.
    Unbound(Binding),
}

/// The unbound number key or ⚡ a refusal is about, and whether the sequence held it alone, which decides the
/// launcher's wording.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binding {
    pub reference: Reference,
    /// Whether the sequence was this one reference and nothing else.
    pub alone: bool,
}

/// A queued number key or ⚡, as a refusal names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reference {
    /// A number key, by digit.
    Key(String),
    /// The ⚡ mark.
    Mark,
}

impl Refusal {
    /// The Templates dialog's binding hint when the refusal is an unbound reference typed outside the launcher, chosen
    /// by the first unbound reference in the queue. The old app could set the ⚡ hint and the number-key hint at once;
    /// the GPUI dialog shows one, and the first is the one the user hit first. `None` for every other refusal, which
    /// opens New with [`Refusal::sentence`] instead.
    pub fn hint(&self) -> Option<Hint> {
        match self {
            Refusal::Unbound(Binding {
                reference: Reference::Key(_),
                ..
            }) => Some(Hint::NumberKey),
            Refusal::Unbound(Binding {
                reference: Reference::Mark,
                ..
            }) => Some(Hint::QuickLaunch),
            Refusal::UnknownWord(_) | Refusal::AmbiguousWord { .. } | Refusal::Missing(_) => None,
        }
    }

    /// The refusal New shows when the sequence was typed outside the launcher, in the old app's words, with every name
    /// and word a host part so the drawer escapes it and isolates its direction:
    ///
    /// - `No template matches "word"`
    /// - `Template word "word" is ambiguous: one, two`
    /// - `Template name is no longer available`
    ///
    /// An unbound reference never reaches New: it opens Templates instead ([`Refusal::hint`]). Should a caller ask
    /// anyway, it gets the launcher's wording for it.
    pub fn sentence(&self) -> Vec<Part> {
        match self {
            Refusal::UnknownWord(word) => {
                vec![words("No template matches \""), host(word), words("\"")]
            }
            Refusal::AmbiguousWord { word, candidates } => {
                let mut parts = vec![
                    words("Template word \""),
                    host(word),
                    words("\" is ambiguous: "),
                ];
                for (index, candidate) in candidates.iter().enumerate() {
                    if index > 0 {
                        parts.push(words(", "));
                    }
                    parts.push(host(candidate));
                }
                parts
            }
            Refusal::Missing(name) => vec![
                words("Template "),
                host(name),
                words(" is no longer available"),
            ],
            Refusal::Unbound(binding) => unbound_sentence(binding),
        }
    }

    /// The refusal the launcher shows beneath search, in the old launcher's words (`list/view.rs`). Word refusals read
    /// as they do outside it. An unbound key or ⚡ is named when the sequence was that reference alone. A key or ⚡ whose
    /// captured template is gone always gets the sentence about the sequence, alone or not: the old app turned every
    /// bound key in a sequence into its template's name before the launcher saw it, so only an unbound reference ever
    /// reached the launcher's by-name wording, and saying "No template is marked ⚡" while ⚡ is still set would be
    /// false.
    pub fn launcher_sentence(&self) -> Vec<Part> {
        match self {
            Refusal::Unbound(binding) => unbound_sentence(binding),
            Refusal::Missing(_) => vec![words(SEQUENCE_UNBOUND_OR_MISSING)],
            Refusal::UnknownWord(_) | Refusal::AmbiguousWord { .. } => self.sentence(),
        }
    }
}

/// The old launcher's sentence for a sequence it could not resolve as a whole.
const SEQUENCE_UNBOUND_OR_MISSING: &str = "A template in this sequence is unbound or missing";

/// The launcher's wording for an unbound reference: by name when the sequence was that reference alone.
fn unbound_sentence(binding: &Binding) -> Vec<Part> {
    let text = match binding {
        Binding {
            reference: Reference::Mark,
            alone: true,
        } => "No template is marked ⚡ for quick launch".to_owned(),
        Binding {
            reference: Reference::Key(key),
            alone: true,
        } => format!("No template is bound to key {key}"),
        Binding { alone: false, .. } => SEQUENCE_UNBOUND_OR_MISSING.to_owned(),
    };
    vec![Part::Words(text)]
}

/// App wording, drawn as is.
fn words(text: &str) -> Part {
    Part::Words(text.to_owned())
}

/// A template name or typed word, drawn as host text.
fn host(text: &str) -> Part {
    Part::Host(text.to_owned())
}

// ---------------------------------------------------------------------------------------------------------------------
// Matching words to names
// ---------------------------------------------------------------------------------------------------------------------

/// Every name in `names` that `word` matches, best tier first: the case-sensitive exact name, then names equal to it
/// in Unicode lowercase, then names it is a lowercase prefix of, catalog order within each tier and each name once.
/// Empty for an empty word. Uncapped, so ambiguity is decided over all of them; [`Sequence::view`] caps the display.
///
/// Lowercase is Rust's `str::to_lowercase`, as SPEC.md says, not full case folding: "STRASSE" does not match "Straße",
/// and "istan" does not match "İstanbul", whose lowercase starts with "i" and a combining dot.
pub fn matching_names<'a, S: AsRef<str>>(word: &str, names: &'a [S]) -> Vec<&'a str> {
    if word.is_empty() {
        return Vec::new();
    }
    let folded = word.to_lowercase();
    let mut tiers: [Vec<&str>; 3] = Default::default();
    for name in names {
        let name = name.as_ref();
        let lower = name.to_lowercase();
        let tier = if name == word {
            0
        } else if lower == folded {
            1
        } else if lower.starts_with(&folded) {
            2
        } else {
            continue;
        };
        if !tiers[tier].contains(&name) {
            tiers[tier].push(name);
        }
    }
    tiers.concat()
}

/// The one template `word` names, by the tiers [`matching_names`] describes: the case-sensitive exact name wins
/// outright, a unique lowercase-exact name wins over longer prefixes, and otherwise the word must be a lowercase prefix
/// of exactly one name. Tab and Enter both use it, so Tab never completes a word Enter would refuse against the same
/// names.
pub fn match_word<'a, S: AsRef<str>>(word: &str, names: &'a [S]) -> Result<&'a str, Refusal> {
    let all = matching_names(word, names);
    if let Some(exact) = all.iter().find(|name| **name == word) {
        return Ok(exact);
    }
    let folded = word.to_lowercase();
    let exact: Vec<&str> = all
        .iter()
        .copied()
        .filter(|name| name.to_lowercase() == folded)
        .collect();
    // With no lowercase-exact name, `all` holds only prefix matches, so it is the winning tier.
    let tier = if exact.is_empty() { all } else { exact };
    match tier.as_slice() {
        [name] => Ok(name),
        [] => Err(Refusal::UnknownWord(word.to_owned())),
        _ => Err(Refusal::AmbiguousWord {
            word: word.to_owned(),
            candidates: tier.iter().map(|name| (*name).to_owned()).collect(),
        }),
    }
}

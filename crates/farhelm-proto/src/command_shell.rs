//! Fill command placeholders without normalizing the user's Bash source.
//!
//! Literal `shell_words` slots remain the contract for the other launch phases. The native
//! Bash grammar only proves that those slots are actual unquoted argument words, rather than
//! quotes, comments, heredoc text or fragments of larger words. Only the proven original spans
//! are replaced. Bash itself remains responsible for interpreting the resulting program.

use std::ops::Range;

use crate::session_launch::RESERVED_PLACEHOLDERS;
use crate::text::shell_quote;

/// Refuse placeholders whose original spelling is not a whole, unquoted Bash argument.
///
/// This supplements the existing literal shape, count and launch-kind checks; it does not
/// replace them. Marker-free programs are not rejected by the Bash grammar, including syntax
/// the parser does not recognize. NUL is always refused, even in comments, because the entire
/// source becomes one executable argument. Native save/admission and spawn both need this check; stored-row
/// decoding must keep using its shape-only validation so old rows remain readable.
pub fn validate_command_placeholders(field: &str, text: &str) -> Result<(), String> {
    marker_spans(field, text).map(|_| ())
}

/// Splice single-quoted literal replacement words into proven original marker spans.
///
/// The callback receives a reserved marker and returns its already-computed literal argv
/// expansion. Empty expansions remove the marker, while empty arguments become `''`. Results
/// are never scanned again: values containing marker text remain literal. All source bytes
/// outside the original marker spans, including whitespace and comments, survive unchanged.
pub fn render_command(
    field: &str,
    text: &str,
    mut replacement: impl FnMut(&str) -> Result<Vec<String>, String>,
) -> Result<String, String> {
    let spans = marker_spans(field, text)?;
    let mut rendered = String::with_capacity(text.len());
    let mut copied = 0;
    for marker in spans {
        rendered.push_str(&text[copied..marker.range.start]);
        let words = replacement(marker.name)?;
        if words.iter().any(|word| word.contains('\0')) {
            return Err(format!("{field}: {} replacement contains NUL", marker.name));
        }
        let mut filled = words
            .iter()
            .map(|word| shell_quote(word))
            .collect::<Vec<_>>()
            .join(" ");
        // Old-style substitutions remove escapes before parsing their contents. Undo each
        // containing stage, innermost first, or an inserted backtick could close an outer
        // substitution despite being single-quoted at the innermost shell's parsing stage.
        for frame in marker.backticks.iter().rev() {
            filled = frame.encode(&filled);
        }
        rendered.push_str(&filled);
        copied = marker.range.end;
    }
    rendered.push_str(&text[copied..]);
    Ok(rendered)
}

/// One replacement target, still in the original source's byte coordinates.
struct Marker {
    range: Range<usize>,
    name: &'static str,
    backticks: Vec<BacktickFrame>,
}

/// The quote-removal stage applied before Bash reparses one backtick interior.
///
/// Escaped nested backticks are ordinary words in the grammar's original-source tree. Decode
/// only this construct, then ask the same maintained grammar to classify the resulting source.
/// Enclosing double quotes additionally remove backslashes before double quotes. Keeping that
/// context on each frame avoids treating escape depth as a sufficient quoting contract.
#[derive(Clone, Copy)]
struct BacktickFrame {
    double_quoted: bool,
}

impl BacktickFrame {
    /// Preserve a literal inner source string through this outer quote-removal stage.
    fn encode(self, source: &str) -> String {
        let mut encoded = String::with_capacity(source.len());
        for ch in source.chars() {
            if matches!(ch, '\\' | '$' | '`') || (self.double_quoted && ch == '"') {
                encoded.push('\\');
            }
            encoded.push(ch);
        }
        encoded
    }
}

/// A parser input plus the original byte range represented by each of its bytes.
///
/// Separate starts and ends matter after removing an escape: the byte after a marker can have
/// a discarded preceding backslash, which must not become part of the marker's original span.
struct Source {
    text: String,
    origins: Vec<Range<usize>>,
    backticks: Vec<BacktickFrame>,
}

impl Source {
    /// Map a nonempty parser node to the original bytes, without claiming contiguity.
    fn original_range(&self, range: Range<usize>) -> Range<usize> {
        self.origins[range.start].start..self.origins[range.end - 1].end
    }

    /// Decode exactly one backtick interior while preserving the source map.
    ///
    /// Command/word/quote/heredoc classification stays in tree-sitter. This only removes the
    /// backslashes Bash consumes before that next parse, including physical continuations.
    fn backtick_interior(&self, range: Range<usize>, frame: BacktickFrame) -> Self {
        let bytes = self.text.as_bytes();
        let mut decoded = Vec::with_capacity(range.len());
        let mut origins = Vec::with_capacity(range.len());
        let mut at = range.start;
        while at < range.end {
            if bytes[at] == b'\\' && at + 1 < range.end {
                let next = bytes[at + 1];
                if next == b'\n' {
                    at += 2;
                    continue;
                }
                if matches!(next, b'$' | b'`' | b'\\') || (frame.double_quoted && next == b'"') {
                    at += 1;
                }
            }
            decoded.push(bytes[at]);
            origins.push(self.origins[at].clone());
            at += 1;
        }
        let mut backticks = self.backticks.clone();
        backticks.push(frame);
        Self {
            // Removing only ASCII escape bytes cannot invalidate UTF-8.
            text: String::from_utf8(decoded).expect("backtick decoding preserves UTF-8"),
            origins,
            backticks,
        }
    }
}

/// Recover safe targets and reject any discrepancy with the existing literal marker slots.
fn marker_spans(field: &str, text: &str) -> Result<Vec<Marker>, String> {
    if text.contains('\0') {
        return Err(format!("{field}: command source contains NUL"));
    }
    // The literal oracle also catches split spellings such as `\{cwd\}` or `'{c'wd'}'`.
    // Its own syntax errors remain the caller's existing shape-validation responsibility.
    // Every supported quote-removal stage only removes bytes. It cannot
    // synthesize either brace, so this bypass does not guess escape depth.
    if !text.contains('{') || !text.contains('}') {
        return Ok(Vec::new());
    }
    let literal = shell_words::split(text).unwrap_or_default();
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&tree_sitter_bash::LANGUAGE.into())
        .map_err(|err| format!("{field}: cannot load Bash placeholder parser: {err}"))?;
    let mut pending = vec![Source {
        text: text.to_owned(),
        origins: (0..text.len()).map(|at| at..at + 1).collect(),
        backticks: Vec::new(),
    }];
    let mut markers = Vec::new();
    let mut comments = Vec::new();
    let mut invalid_syntax = false;
    let mut literal_marker = false;
    while let Some(source) = pending.pop() {
        // Every backtick reparse has its own quote-removal stage. Inspect
        // that stage's literal words too: a glued escaped marker may only
        // become visible here, while only root slots authorize replacement.
        let stage_words = shell_words::split(&source.text).unwrap_or_default();
        for &name in RESERVED_PLACEHOLDERS {
            for word in &stage_words {
                if word.contains(name) {
                    literal_marker = true;
                    if word != name {
                        return Err(placement_error(field, name));
                    }
                }
            }
        }
        let tree = parser
            .parse(&source.text, None)
            .ok_or_else(|| format!("{field}: cannot parse Bash placeholder placement"))?;
        // A cursor avoids a Rust call frame per nested command. Backtick interiors are the
        // only transformed inputs and are reparsed separately, never walked undecoded.
        let mut cursor = tree.root_node().walk();
        loop {
            let node = cursor.node();
            let range = node.byte_range();
            let raw = &source.text[range.clone()];
            let mut descend = true;
            if node.kind() == "command_substitution"
                && (raw.starts_with('`') || raw.starts_with("$`"))
            {
                // Error recovery may supply a missing closing delimiter. An opaque subtree
                // still needs real boundaries before any decoded children can certify spans.
                if !raw.ends_with('`')
                    || node
                        .child_count()
                        .checked_sub(1)
                        .and_then(|at| node.child(at))
                        .is_none_or(|last| last.is_missing() || last.kind() != "`")
                {
                    invalid_syntax = true;
                } else {
                    let frame = BacktickFrame {
                        double_quoted: inside_double_quotes(node),
                    };
                    let prefix = if raw.starts_with("$`") { 2 } else { 1 };
                    pending
                        .push(source.backtick_interior(range.start + prefix..range.end - 1, frame));
                }
                descend = false;
            } else if node.is_error() || node.is_missing() {
                invalid_syntax = true;
            } else if node.kind() == "comment" && !range.is_empty() {
                comments.push(source.original_range(range));
                descend = false;
            } else if is_argument(node, &cursor)
                && let Some(&name) = RESERVED_PLACEHOLDERS.iter().find(|&&marker| raw == marker)
            {
                let original = source.original_range(range);
                if &text[original.clone()] != name {
                    return Err(placement_error(field, name));
                }
                markers.push(Marker {
                    range: original,
                    name,
                    backticks: source.backticks.clone(),
                });
                descend = false;
            }
            if descend && cursor.goto_first_child() {
                continue;
            }
            loop {
                if cursor.goto_next_sibling() {
                    break;
                }
                if !cursor.goto_parent() {
                    break;
                }
            }
            if cursor.node() == tree.root_node() {
                break;
            }
        }
    }
    markers.sort_by_key(|marker| marker.range.start);
    // Braces alone are not markers, and established comments remain inert.
    // Unknown grammar must not reject an otherwise untouched marker-free
    // program merely because discovery parsed it to inspect a backtick stage.
    let raw_marker = RESERVED_PLACEHOLDERS.iter().any(|name| {
        text.match_indices(name).any(|(at, _)| {
            !comments
                .iter()
                .any(|comment| comment.start <= at && at + name.len() <= comment.end)
        })
    });
    if invalid_syntax && (literal_marker || raw_marker || !markers.is_empty()) {
        return Err(format!(
            "{field}: cannot establish Bash placeholder placement; write each placeholder unquoted as its own word"
        ));
    }
    for &name in RESERVED_PLACEHOLDERS {
        let literal_count = literal.iter().filter(|word| word.as_str() == name).count();
        if markers.iter().filter(|marker| marker.name == name).count() != literal_count {
            return Err(placement_error(field, name));
        }
        for (at, _) in text.match_indices(name) {
            let range = at..at + name.len();
            if !markers.iter().any(|marker| marker.range == range)
                && !comments
                    .iter()
                    .any(|comment| comment.start <= range.start && range.end <= comment.end)
            {
                return Err(placement_error(field, name));
            }
        }
    }
    Ok(markers)
}

/// Distinguish argument fields from superficially similar words in expressions and redirects.
fn is_argument(node: tree_sitter::Node<'_>, cursor: &tree_sitter::TreeCursor<'_>) -> bool {
    let Some(parent) = node.parent() else {
        return false;
    };
    (cursor.field_name() == Some("argument")
        && matches!(parent.kind(), "command" | "heredoc_redirect"))
        || (node.kind() == "command_name" && parent.kind() == "command")
}

/// Find the quoting stage containing a backtick, stopping at a fresh shell grammar scope.
fn inside_double_quotes(node: tree_sitter::Node<'_>) -> bool {
    let mut ancestor = node.parent();
    while let Some(parent) = ancestor {
        match parent.kind() {
            "string" => return true,
            "command_substitution" | "process_substitution" | "heredoc_body" => return false,
            _ => ancestor = parent.parent(),
        }
    }
    false
}

/// Keep all placement failures actionable, including alternate quoted/escaped spellings.
fn placement_error(field: &str, marker: &str) -> String {
    format!("{field}: write {marker} unquoted as its own word")
}

#[cfg(test)]
mod tests {
    use super::{BacktickFrame, Source, render_command, validate_command_placeholders};
    use crate::text::shell_quote;

    /// Shell rendering must preserve all non-marker bytes, including multibyte offsets and
    /// comments; reconstructing source from argv would silently change these programs.
    #[farhelm_testtrace::test]
    fn original_bytes_survive_and_values_are_not_rescanned() {
        let source = "écho $HOME \\\n{cwd}\n# {cwd} stays a comment\nagent {farhelm_args} ; cat <<EOF\nplain body\nEOF\n";
        let rendered = render_command("command", source, |marker| {
            Ok(match marker {
                "{cwd}" => vec!["a'{cwd}$`\\\n".to_owned()],
                "{farhelm_args}" => vec![String::new(), "--hook".to_owned()],
                _ => panic!("unexpected marker"),
            })
        })
        .unwrap();
        assert_eq!(
            rendered,
            "écho $HOME \\\n'a'\\''{cwd}$`\\\n'\n# {cwd} stays a comment\nagent '' '--hook' ; cat <<EOF\nplain body\nEOF\n"
        );
        assert_eq!(
            render_command("command", "agent {farhelm_args}", |_| Ok(vec![])).unwrap(),
            "agent "
        );
        assert_eq!(
            render_command("command", "agent {cwd} {cwd}", |_| Ok(vec![
                "two words".into()
            ]))
            .unwrap(),
            "agent 'two words' 'two words'"
        );
    }

    /// Quoting is safe only in real argument positions. A heredoc is especially important:
    /// single quotes in its body do not protect an inserted dollar-command substitution.
    #[farhelm_testtrace::test]
    fn quoted_glued_escaped_and_nonargument_markers_are_refused() {
        for source in [
            "agent '{cwd}'",
            "agent \"{cwd}\"",
            "agent $'{cwd}'",
            "agent $\"{cwd}\"",
            "agent \\{cwd\\}",
            "agent '{c'wd'}'",
            "printf '%s' pre'{c'wd'}'",
            "printf '%s' \\{cwd\\}/child",
            "agent {cwd} pre'{c'wd'}'",
            "agent {cwd} \\{cwd\\}/child",
            r": ` printf '%s' pre\\{cwd\\}/child `",
            r"printf '%s' {cwd} ; : ` printf '%s' pre\\{cwd\\}/child `",
            r": ` : \` printf '%s' pre\\\\{cwd\\\\}/child \` `",
            "agent x{cwd}",
            "agent {cwd}/child",
            "agent {cwd}''",
            "agent {cwd};",
            "agent $(printf x) --dir={cwd}",
            "cat <<EOF\n{cwd}\nEOF\n",
            "cat <<'{cwd}'\nbody\n{cwd}\n",
            "x={cwd} agent",
            "cat > {cwd}",
            "case x in {cwd} ) : ;; esac",
        ] {
            let error = validate_command_placeholders("command", source).expect_err(source);
            assert!(
                error.contains("unquoted as its own word"),
                "{source:?}: {error}"
            );
        }
    }

    /// Unrelated shell constructs cannot become a blanket rejection class. Nested markers
    /// still obey the unchanged literal whole-element slots used by the other launch phases.
    #[farhelm_testtrace::test]
    fn substitutions_and_unrelated_heredocs_keep_their_real_word_contexts() {
        for source in [
            "agent $(printf x) {cwd}",
            "echo $( agent {cwd} )",
            "echo $(case x in x) agent {cwd} ;; esac)",
            "echo $(cat <<EOF\n)\nEOF\nagent {cwd}\n)",
            "cat <<EOF {cwd}\nbody\nEOF\n",
            "cat <<EOF\nbody\nEOF\nagent {cwd}",
            "echo >( agent {cwd} )",
            ": ` printf '%s' {cwd} `",
            r": ` : \` printf '%s' {cwd} \` `",
        ] {
            validate_command_placeholders("command", source)
                .unwrap_or_else(|error| panic!("{source:?}: {error}"));
        }
        // Bash recognizes this argument, but shell_words has no whole marker slot to fill.
        assert!(validate_command_placeholders("command", "echo $(agent {cwd})").is_err());
    }

    /// Marker-free source must not acquire a new grammar gate, while NUL can never be sent
    /// in bash's -c argument even when a literal parser would discard it as a comment.
    #[farhelm_testtrace::test]
    fn marker_free_source_has_no_new_grammar_gate_but_rejects_nul() {
        for source in [
            "cat <<EOF\n$(x)\nEOF\n",
            "syntax ' not even parsed",
            "syntax ' { not even parsed }",
            "; { }\n# {cwd} remains a comment",
            "écho ~ $VAR | cat >out &",
        ] {
            assert_eq!(
                render_command("command", source, |_| panic!("no marker")).unwrap(),
                source
            );
        }
        assert!(
            validate_command_placeholders("command", "agent # hidden\0")
                .unwrap_err()
                .contains("NUL")
        );
        assert!(
            render_command("command", "agent {cwd}", |_| Ok(vec!["bad\0value".into()]))
                .unwrap_err()
                .contains("NUL")
        );
    }

    /// The actual innermost Bash command must receive hostile values byte for byte. Its
    /// output goes directly to stderr so command-substitution trimming and field splitting
    /// cannot hide damage. Each case owns its child process and changes no test environment.
    #[farhelm_testtrace::test]
    fn real_bash_keeps_replacements_literal_through_backtick_stages() {
        for source in [
            "printf '%s' {cwd} >&2",
            ": $( printf '%s' {cwd} >&2 )",
            ": ` printf '%s' {cwd} >&2 `",
            r": ` : \` printf '%s' {cwd} >&2 \` `",
        ] {
            for value in [
                "",
                "a b",
                "quote'\"",
                "$(printf injected)",
                "`printf injected`",
                "back\\slash",
                "line\nbreak\n",
                "{cwd}",
            ] {
                let rendered =
                    render_command("command", source, |_| Ok(vec![value.to_owned()])).unwrap();
                let output = std::process::Command::new("bash")
                    .args(["-c", &rendered])
                    .output()
                    .unwrap();
                assert!(output.status.success(), "{source:?}: {:?}", output.stderr);
                assert!(output.stdout.is_empty(), "{source:?}: {:?}", output.stdout);
                assert_eq!(
                    output.stderr,
                    value.as_bytes(),
                    "source={source:?}, rendered={rendered:?}"
                );
            }
        }
    }

    /// An enclosing double-quoted string changes backtick quote removal. Such a marker is
    /// outside the current literal-slot contract, but the private codec must still describe
    /// that parsing stage correctly rather than depend on an unreachable-context assumption.
    #[farhelm_testtrace::test]
    fn real_bash_confirms_double_quoted_backtick_codec() {
        let frame = BacktickFrame {
            double_quoted: true,
        };
        assert!(
            validate_command_placeholders("command", ": \"` printf '%s' {cwd} >&2 `\"").is_err()
        );
        for value in [
            "quote'\"",
            "back\\\"slash",
            "$(printf injected)",
            "`printf injected`",
            "line\n\\\nbreak\n",
        ] {
            let inner = format!("printf '%s' {} >&2", shell_quote(value));
            let encoded = frame.encode(&inner);
            let source = Source {
                origins: (0..encoded.len()).map(|at| at..at + 1).collect(),
                text: encoded,
                backticks: Vec::new(),
            };
            let decoded = source.backtick_interior(0..source.text.len(), frame);
            assert_eq!(decoded.text, inner);
            let program = format!(": \"` {} `\"", source.text);
            let output = std::process::Command::new("bash")
                .args(["-c", &program])
                .output()
                .unwrap();
            assert!(output.status.success(), "{program:?}: {:?}", output.stderr);
            assert!(output.stdout.is_empty(), "{program:?}: {:?}", output.stdout);
            assert_eq!(output.stderr, value.as_bytes(), "{program:?}");
        }
    }
}

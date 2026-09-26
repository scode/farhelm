//! How `farhelm agent` shows a reply to a person: the aligned listing
//! tables, and the escaping every piece of peer-supplied text goes through
//! before it reaches a terminal.
//!
//! Everything printed here arrived from somewhere this process does not
//! control: session titles and working directories typed on other hosts,
//! host names, and error prose written by a helm or a target supervisor.
//! That is why the table cells, the confirmation lines `main` prints, and
//! the error messages [`crate::agent_client`] raises all share one escaping
//! floor ([`safe_cell`]) and one size bound, rather than each output path
//! deciding for itself what is safe to print.

use farhelm_proto::AgentReply;

/// The one-line warning a cut listing owes its reader, or `None` for a
/// complete one.
///
/// Separate from the table because it does not belong on stdout: the table
/// is the machine-readable answer and this is a statement about that
/// answer's completeness. It exists at all because a truncated listing is
/// shaped exactly like a whole one, so without it "no such session" and
/// "past the cut" are the same output.
pub(crate) fn truncation_notice(reply: &AgentReply) -> Option<String> {
    match reply {
        AgentReply::Sessions {
            sessions,
            truncated: true,
            ..
        } => Some(format!(
            "warning: this is not the whole fleet; the listing was cut at {} sessions",
            sessions.len()
        )),
        _ => None,
    }
}

/// One listing as a plain aligned table on stdout.
///
/// A table rather than JSON because the consumer is a language model
/// reading its own shell output: columns survive being quoted into a
/// conversation, and an agent that wanted structure would be parsing prose
/// out of `message` on the failure path anyway. The `*` column is the one
/// piece of information that has no other spelling — which row is the
/// asking session, and which host it is on.
///
/// Only ever called with the reply to `Hosts`, `Sessions`, or `Profiles` —
/// the four lifecycle verbs print their own one-line confirmation instead
/// (see `main`'s `Rename`/`Stop`/`Restart` arms) and the two creating verbs
/// print an id on stdout with their confirmation on stderr — which is why
/// the lifecycle and `Created` tags are an ERROR here rather than tables of
/// their own.
///
/// A `Result` rather than a panic on those tags, deliberately. The
/// precondition is real and holds today, but it is a fact about this
/// program's own dispatch rather than anything the type system carries, and
/// a `unreachable!()` turns a future routing mistake into a CLI that
/// aborts. Every other "this is not the shape I expected" case in this file
/// — a reply whose tag disagrees with its question, a lifecycle verb
/// answered with the wrong variant — already ends in a `bail!`, and this
/// belongs in the same family.
///
/// Returns the text rather than printing it so the shape is testable
/// without a process.
pub(crate) fn render_agent_reply(reply: &AgentReply) -> anyhow::Result<String> {
    match reply {
        AgentReply::Hosts { hosts, .. } => {
            let mut rows = vec![vec![
                String::new(),
                "ID".to_string(),
                "NAME".to_string(),
                "KIND".to_string(),
                "STATE".to_string(),
            ]];
            rows.extend(hosts.iter().map(|host| {
                vec![
                    marker(host.current),
                    host.id.clone(),
                    host.name.clone(),
                    host.kind.clone(),
                    host.state.clone(),
                ]
            }));
            // NAME (column 1) is printed WHOLE, exempt from the clamp every
            // other non-final column takes, because this column is not a
            // description of a host — it is the SELECTOR for one. `create`
            // and `clone` take `--host <NAME>` and match it exactly, so a
            // name the listing cut at 48 characters and marked with `…` is
            // a host an agent can see and can never target. The clamp's
            // amplification argument does not transfer here either: host
            // names are operator-registered ssh destinations, one per
            // machine in a fleet, not the per-session user text
            // `MAX_CELL_WIDTH` was written for. The sessions table below
            // keeps the clamp on every one of its columns for exactly that
            // reason.
            Ok(aligned(&rows, &[1, 2]))
        }
        AgentReply::Sessions { sessions, .. } => {
            let mut rows = vec![vec![
                String::new(),
                "ID".to_string(),
                "HOST".to_string(),
                "TITLE".to_string(),
                "CWD".to_string(),
                "AGENT".to_string(),
                "STATUS".to_string(),
                "OFFER".to_string(),
            ]];
            rows.extend(sessions.iter().map(|session| {
                vec![
                    marker(session.current),
                    session.id.clone(),
                    host_cell(session),
                    session.title.clone(),
                    session.cwd.clone(),
                    session.agent.clone(),
                    session_status_cell(session),
                    restart_offer_cell(session.restart_offer).to_string(),
                ]
            }));
            Ok(aligned(&rows, &[]))
        }
        AgentReply::Profiles { profiles, .. } => {
            let mut rows = vec![vec![
                "ID".to_string(),
                "NAME".to_string(),
                "BUILTIN".to_string(),
            ]];
            rows.extend(profiles.iter().map(|profile| {
                vec![
                    profile.id.clone(),
                    profile.name.clone(),
                    profile.builtin.to_string(),
                ]
            }));
            Ok(aligned(&rows, &[0, 1]))
        }
        // Refused rather than rendered: a lifecycle or creating reply has
        // one row and no table to be, and printing an empty one would read
        // as an empty fleet. `main` never routes one here — see this
        // function's docs for why that is stated as an error and not
        // asserted.
        AgentReply::Session { .. }
        | AgentReply::Restarted { .. }
        | AgentReply::Stopped {}
        | AgentReply::Created { .. }
        | AgentReply::ResolvedProfile { .. } => {
            anyhow::bail!("only discovery listings are rendered as a table")
        }
    }
}

/// The mode spelling a session row offers to `farhelm agent restart`.
///
/// It is deliberately only the non-secret enum: the command, template, and
/// captured locator that implement an offer stay on the target supervisor.
fn restart_offer_cell(offer: farhelm_proto::RestartOffer) -> &'static str {
    match offer {
        farhelm_proto::RestartOffer::FreshOnly => "fresh",
        farhelm_proto::RestartOffer::Resume => "resume",
        farhelm_proto::RestartOffer::FallbackTemplate => "fallback-template",
    }
}

/// The HOST cell: the name, or an explicit stand-in when the helm had none
/// to vouch for.
///
/// The absent case is unreachable from a LISTING today — `AgentSession::
/// host` is only ever `None` on a mutating verb's own reply, whose host is
/// pinned to the connection the mutation routed through and can stop being
/// current before the row is projected — and spelled out anyway rather than
/// defaulted to an empty cell. An empty cell in a column of host names
/// reads as a rendering bug; this reads as what it is, and pairs with the
/// `(stale)` the status cell puts on the same row.
///
/// Shared with the creating verbs' stderr confirmation, which is not a
/// table at all: "created … on (unknown)" is an odd sentence, and it is
/// still the honest one — the session exists and the helm cannot name the
/// machine it landed on, which a reader must not be told is a machine
/// called nothing.
pub(crate) fn host_cell(session: &farhelm_proto::AgentSession) -> String {
    session
        .host
        .clone()
        .unwrap_or_else(|| "(unknown)".to_string())
}

/// The STATUS cell, annotated when the helm's reading may be stale.
fn session_status_cell(session: &farhelm_proto::AgentSession) -> String {
    if session.stale {
        return format!("{} (stale)", session.status);
    }
    session.status.clone()
}

/// The "this one is you" column: `*` for the asking session and its host,
/// empty otherwise.
fn marker(current: bool) -> String {
    if current { "*" } else { "" }.to_string()
}

/// The widest a non-final column is allowed to get.
///
/// This bound is what keeps output linear in the ROW COUNT rather than in
/// row count times the longest field. Alignment pads every cell in a column
/// to the widest one, and session fields are user text bounded only by the
/// supervisor's create-time cap (tens of kilobytes), so one pathological
/// title in a middle column would otherwise add that many spaces to every
/// other row — turning a valid, bounded reply into hundreds of megabytes
/// before a single byte is printed.
///
/// Forty-eight is chosen to fit the values that actually matter whole —
/// session ids, host names, ordinary titles, most working directories — on
/// a terminal that can still show the columns after it.
const MAX_CELL_WIDTH: usize = 48;

/// Pad every column to its widest cell, one row per line, with every cell
/// first made safe and bounded.
///
/// Three things happen here, and each is load-bearing:
///
/// **Escaping.** Every cell is arbitrary text from somewhere else on the
/// fleet — titles, working directories, host names, status words — printed
/// straight to a terminal. A newline in one cell forges a row; a tab shifts
/// the columns; an ESC introduces a control sequence that can repaint the
/// screen, hide what is above it, or reach terminal features that have
/// nothing to do with printing. So control characters become visible
/// escapes, and one cell can only ever produce one line.
///
/// **Clamping.** Non-final columns are cut to [`MAX_CELL_WIDTH`] with a
/// trailing `…`, for the amplification reason that constant documents. The
/// final column is neither padded nor clamped: nothing follows it, so it
/// costs its own length and no more. Columns named in `verbatim` are
/// exempt, which is a statement about what the column IS rather than a
/// formatting preference — see the hosts listing's own call site.
///
/// **Widths in `char`s, not bytes.** A title or directory is arbitrary user
/// text, and byte counting would misalign every row after the first
/// non-ASCII one. It remains an approximation — a wide CJK glyph occupies
/// two terminal cells and counts as one char here — and that is the right
/// approximation for this surface: the reader is a model parsing columns,
/// not a person eyeballing a grid, and the alternative is a Unicode
/// width-table dependency for a debug-shaped listing.
///
/// The last column is never padded, so no line carries trailing spaces.
fn aligned(rows: &[Vec<String>], verbatim: &[usize]) -> String {
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    // Sanitized ONCE, before anything measures: a width taken from the raw
    // text and then applied to the escaped text would misalign every row
    // that contained anything to escape.
    let rows: Vec<Vec<String>> = rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(column, cell)| {
                    let cell = safe_cell(cell);
                    if column + 1 == row.len() || verbatim.contains(&column) {
                        cell
                    } else {
                        clamp(cell)
                    }
                })
                .collect()
        })
        .collect();
    let widths: Vec<usize> = (0..columns)
        .map(|column| {
            rows.iter()
                .filter_map(|row| row.get(column))
                .map(|cell| cell.chars().count())
                .max()
                .unwrap_or(0)
        })
        .collect();
    let mut out = String::new();
    for row in &rows {
        let mut line = String::new();
        for (column, cell) in row.iter().enumerate() {
            if column + 1 == row.len() {
                line.push_str(cell);
            } else {
                line.push_str(cell);
                let pad = widths[column].saturating_sub(cell.chars().count());
                line.extend(std::iter::repeat_n(' ', pad + 1));
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// One field as a quoted, unambiguously delimited token.
///
/// For the rename confirmation, whose title is the one place this CLI puts
/// peer-supplied text inside literal quotes on stdout. [`safe_cell`] alone
/// was not enough there: it neutralizes control characters but leaves a `"`
/// as a `"`, so a title of `x" and then some` printed as
/// `renamed s1 to "x" and then some"` — three quotes, and anything reading
/// the line for a quoted field sees the title end where the attacker chose.
///
/// The two escapes happen BEFORE `safe_cell`, and the order is what makes
/// the result decodable. Escaping first means a literal backslash in the
/// title is already doubled by the time `safe_cell` writes its own
/// backslash escapes, so `\n` in the output is unambiguously a newline that
/// was escaped and never a title that literally contained a backslash and
/// an n.
pub(crate) fn quoted(field: &str) -> String {
    let mut escaped = String::with_capacity(field.len());
    for c in field.chars() {
        if c == '"' || c == '\\' {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    format!("\"{}\"", safe_cell(&escaped))
}

/// One cell as a single printable line.
///
/// Unsafe means [`farhelm_proto::text::is_presentation_unsafe`]: the set every
/// surface that shows peer-supplied text shares, so a character the browser
/// escapes is never printed raw here. Table cells and peer-supplied error
/// prose both come through this function.
///
/// Every unsafe character is replaced by a visible escape rather than
/// dropped, so a cell that contained one still says so — a silently
/// stripped newline turns two forged rows into one plausible row, which is
/// worse than an ugly one.
pub(crate) fn safe_cell(cell: &str) -> String {
    let mut out = String::with_capacity(cell.len());
    for ch in cell.chars() {
        match ch {
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if farhelm_proto::text::is_presentation_unsafe(c) => {
                if c.is_control() {
                    // Cc is Unicode's C0, DEL, and C1 category, all of
                    // which fit in two hex digits.
                    out.push_str(&format!("\\x{:02x}", c as u32));
                } else {
                    out.push_str(&format!("\\u{{{:04x}}}", c as u32));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Cut `cell` to [`MAX_CELL_WIDTH`] characters, marking the cut with `…`.
///
/// The ellipsis replaces the last kept character rather than being appended
/// to it, so a clamped cell is exactly the width the bound names — a cell
/// that could exceed its own limit would defeat the point of having one.
fn clamp(cell: String) -> String {
    clamp_to(cell, MAX_CELL_WIDTH)
}

/// The longest a peer-written error message is ever printed at,
/// before this process's own `Result`-printing takes over.
///
/// Sized for a SENTENCE rather than a table cell, unlike [`MAX_CELL_WIDTH`]:
/// an error is prose meant to be read whole, not a column meant to stay
/// aligned with its neighbors, so it gets a far more generous cap of its
/// own rather than reusing one built for a different job.
const MAX_ERROR_MESSAGE_CHARS: usize = 4096;

/// A peer-supplied error message, made safe for the same reason
/// [`safe_cell`] exists: escaped so an embedded control character cannot
/// forge terminal output, and bounded so a pathologically large refusal
/// cannot flood the screen.
///
/// Exists because an `AgentOutcome::Err`'s `message` — unlike every
/// successful confirmation's fields, which already go through
/// [`safe_cell`] — used to reach `anyhow::bail!` (and from there, this
/// process's own unescaped `Result` printer) with neither protection. That
/// was a latent gap even for the helm's own fixed refusal strings, and the
/// lifecycle verbs made it a real one: a rename/stop/restart refusal can
/// now carry a TARGET supervisor's own free-text prose (a rejected title,
/// say), which this process never validated on the way out. A supervisor's
/// own `ControlMsg::Error` refusal gets the same treatment, for `spawn` and
/// `agent` alike, in [`crate::agent_client::one_shot_request`].
pub(crate) fn safe_error_message(message: &str) -> String {
    clamp_to(safe_cell(message), MAX_ERROR_MESSAGE_CHARS)
}

/// [`clamp`]'s general form, for a width other than [`MAX_CELL_WIDTH`].
///
/// `clamp` itself is left as the table-rendering path's own name for this
/// same operation at its own fixed width, rather than folded into this one
/// with an extra argument at every call site.
fn clamp_to(cell: String, width: usize) -> String {
    if cell.chars().count() <= width {
        return cell;
    }
    let mut out: String = cell.chars().take(width - 1).collect();
    out.push('…');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------
    // `farhelm agent`'s table. The process-level contract lives in
    // tests/agent_cli.rs; what follows is the rendering itself, where an
    // exact expected string is cheap and a spawned binary is not.
    // ---------------------------------------------------------------

    fn agent_session(id: &str, title: &str) -> farhelm_proto::AgentSession {
        farhelm_proto::AgentSession {
            id: id.to_string(),
            host_id: "1".to_string(),
            host: Some("h".to_string()),
            title: title.to_string(),
            cwd: "/w".to_string(),
            agent: "claude".to_string(),
            status: "running".to_string(),
            current: false,
            restart_offer: Default::default(),
            stale: false,
        }
    }

    fn sessions(rows: Vec<farhelm_proto::AgentSession>) -> AgentReply {
        AgentReply::Sessions {
            caller_host_id: "host-local".to_string(),
            sessions: rows,
            truncated: false,
        }
    }

    /// Spec: column widths are counted in characters, so a multibyte but
    /// single-width character does not shift the columns after it.
    ///
    /// The formatter deliberately uses `chars().count()` instead of
    /// `len()`, and nothing else notices the difference: every other table
    /// fixture in this repo is ASCII, where the two agree exactly. A
    /// regression to byte length would misalign every row containing an
    /// accented name or title while all existing tests stayed green.
    ///
    /// Wide CJK glyphs are deliberately kept out of this case. They are a
    /// different problem with a different right answer (display width, not
    /// character count), and folding them in here would turn one regression
    /// test into an argument about which approximation is being pinned.
    #[farhelm_testtrace::test]
    fn column_widths_count_characters_not_bytes() {
        // "café" is 4 characters and 5 bytes; "tea" is 3 of each. Under
        // byte counting the first row's TITLE column would be padded one
        // column too wide and CWD would not line up.
        let rendered = render_agent_reply(&sessions(vec![
            agent_session("s1", "café"),
            agent_session("s2", "tea"),
        ]))
        .expect("a sessions listing renders as a table");
        assert_eq!(
            rendered,
            [
                " ID HOST TITLE CWD AGENT  STATUS  OFFER",
                " s1 h    café  /w  claude running fresh",
                " s2 h    tea   /w  claude running fresh",
                "",
            ]
            .join("\n")
        );
    }

    /// Spec: control characters in a cell become visible escapes, so no
    /// value from the fleet can forge a row or drive the terminal.
    ///
    /// Every dynamic cell here is text from somewhere else on the fleet,
    /// printed straight to a terminal a person and a model are both reading.
    /// A newline forges a row that looks exactly like a real one; an ESC
    /// opens a control sequence that can repaint the screen, hide the lines
    /// above it, or reach terminal features that have nothing to do with
    /// printing. Escaping rather than stripping is deliberate: a silently
    /// removed newline turns two forged rows into one plausible row.
    #[farhelm_testtrace::test]
    fn control_characters_in_a_cell_are_escaped_into_one_visible_line() {
        let rendered = render_agent_reply(&sessions(vec![agent_session(
            "s1",
            "real\n  s2 forged\ttab\x1b[31m\u{2028}\u{202e}line",
        )]))
        .expect("a sessions listing renders as a table");
        assert_eq!(
            rendered.lines().count(),
            2,
            "a cell must never produce a second row: {rendered:?}"
        );
        assert!(rendered.contains("real\\n"), "{rendered:?}");
        assert!(rendered.contains("forged\\ttab"), "{rendered:?}");
        assert!(rendered.contains("\\x1b[31m"), "{rendered:?}");
        assert!(rendered.contains("\\u{2028}\\u{202e}"), "{rendered:?}");
        assert!(
            !rendered.contains('\x1b'),
            "no raw ESC may reach the terminal: {rendered:?}"
        );
        assert!(!rendered.contains('\u{2028}'), "{rendered:?}");
        assert!(!rendered.contains('\u{202e}'), "{rendered:?}");
    }

    /// Spec: invisible characters in a cell become visible escapes too, so
    /// two different fleet values never print identically.
    ///
    /// These are not controls and do not break the table, which is how this
    /// path came to print them raw while the browser and the helm's audit log
    /// escaped them: a title with a zero-width space, a soft hyphen, or a
    /// byte-order mark in it reads exactly like the title without one, so a
    /// model choosing a session by title could be pointed at the wrong one.
    #[farhelm_testtrace::test]
    fn invisible_characters_in_a_cell_are_escaped() {
        let rendered = render_agent_reply(&sessions(vec![agent_session(
            "s1",
            "a\u{200b}b\u{00ad}\u{feff}\u{061c}\u{2060}",
        )]))
        .expect("a sessions listing renders as a table");
        assert!(
            rendered.contains("a\\u{200b}b\\u{00ad}\\u{feff}\\u{061c}\\u{2060}"),
            "{rendered:?}"
        );
        for ch in ['\u{200b}', '\u{00ad}', '\u{feff}', '\u{061c}', '\u{2060}'] {
            assert!(
                !rendered.contains(ch),
                "U+{:04X} must not print as itself: {rendered:?}",
                ch as u32
            );
        }
    }

    /// Spec: a non-final column is cut to [`MAX_CELL_WIDTH`] with a `…`.
    ///
    /// This is a resource bound, not a cosmetic one. Alignment pads every
    /// cell of a column to the widest one in it, and session titles, paths
    /// and invocations are user text bounded only by the supervisor's
    /// create-time cap — so one long value in a middle column multiplies by
    /// the row count into an output far larger than the reply that produced
    /// it. The final restart-offer vocabulary needs no arbitrary-text
    /// width exception.
    #[farhelm_testtrace::test]
    fn non_final_columns_are_clamped() {
        let long = "x".repeat(MAX_CELL_WIDTH * 3);
        let mut row = agent_session("s1", &long);
        row.status = long.clone();
        let rendered = render_agent_reply(&sessions(vec![row]))
            .expect("a sessions listing renders as a table");
        let title = rendered
            .lines()
            .nth(1)
            .expect("a data row")
            .split_whitespace()
            .nth(2)
            .expect("the TITLE cell");
        assert_eq!(title.chars().count(), MAX_CELL_WIDTH);
        assert!(title.ends_with('…'), "the cut must be marked: {title}");
        assert!(!rendered.contains(&long), "a non-final cell is bounded");
    }
    /// Spec: a truncated listing produces a warning naming the cut; a
    /// complete one produces none.
    ///
    /// The notice is the only thing distinguishing a partial fleet from a
    /// whole one — the table itself is shaped identically either way, so
    /// without it "no such session" and "past the cut" are the same answer.
    #[farhelm_testtrace::test]
    fn a_truncated_listing_warns_and_a_complete_one_does_not() {
        assert!(truncation_notice(&sessions(vec![agent_session("s1", "t")])).is_none());
        let notice = truncation_notice(&AgentReply::Sessions {
            caller_host_id: "host-local".to_string(),
            sessions: vec![agent_session("s1", "t")],
            truncated: true,
        })
        .expect("a truncated listing must say so");
        assert!(notice.contains("not the whole fleet"), "{notice}");
        assert!(notice.contains('1'), "the notice names the count: {notice}");
    }
}

//! Whether an address names the destination.
//!
//! Comparison is strict on purpose: http against https, an added `www.`, an extra query or fragment, `%7e` against
//! `~`, and a shortened address all differ. The URL parser's own normalization (case, IDNA, default port, dot
//! segments) is the only leeway, plus one trailing path `/`, the case of percent-escape hex digits and an empty `?` or
//! `#`. The one exception is a bare site name (`github.com`), which matches any destination on that host.
//!
//! An address is judged in two forms, as written and trimmed of trailing decoration, and matches when either does:
//! `https://a.test/guide.` at the end of a sentence names `https://a.test/guide`.

use super::{Address, Form, MismatchKind};
use url::Url;

/// Why `address` doesn't name `target`, or `None` when it does.
///
/// When both forms fail, the reason is the reported form's ([`reported`]): disguised, else credentials when it has
/// userinfo, else a plain mismatch. The trimmed form is a prefix of the raw one, so it is disguised or has userinfo
/// only when the raw form does too; reading the reason from it keeps honest trailing punctuation that folding changes
/// (`。`, `，`, `…` after a host) from being reported as a look-alike.
pub(super) fn mismatch(label: &str, address: &Address, target: &Url) -> Option<MismatchKind> {
    let forms = [
        (&address.span, &address.forms[0]),
        (&address.trimmed, &address.forms[1]),
    ];
    if forms
        .iter()
        .any(|(range, form)| matches(&label[(*range).clone()], address, form, target))
    {
        return None;
    }
    let (_, form) = reported(address);
    Some(if form.disguised {
        MismatchKind::Disguised
    } else if form.userinfo {
        MismatchKind::Credentials
    } else {
        MismatchKind::Mismatch
    })
}

/// The form a mismatch is reported for: the trimmed one, unless trimming left nothing.
fn reported(address: &Address) -> (&std::ops::Range<usize>, &Form) {
    if address.trimmed.is_empty() {
        (&address.span, &address.forms[0])
    } else {
        (&address.trimmed, &address.forms[1])
    }
}

/// The URL parser's serialization of the reported form of `address`, when it parses: what the hover marks the
/// destination's difference against.
pub(super) fn reported_url(label: &str, address: &Address, target: &Url) -> Option<String> {
    let (range, _) = reported(address);
    parse(&label[range.clone()], address.written, target).map(String::from)
}

/// Whether one form of an address names `target`. A disguised form, or one with userinfo, never does, whatever it
/// parses to: the label then reads as one place while a browser would read another, or reads the same only because
/// recognition had to clean it up.
fn matches(text: &str, address: &Address, form: &Form, target: &Url) -> bool {
    if form.disguised || form.userinfo {
        return false;
    }
    let Some(url) = parse(text, address.written, target) else {
        return false;
    };
    if address.host_only {
        url.host_str() == target.host_str()
    } else {
        comparable(&url) == comparable(target)
    }
}

/// Parse one form of an address. An address with its own scheme parses as written; any other borrows only the
/// destination's scheme, never its authority (`https:` before `//intranet/`, `https://` before `github.com/x`). An
/// address that doesn't parse is a plain mismatch, not a reason to stay quiet.
fn parse(text: &str, written: bool, target: &Url) -> Option<Url> {
    if written {
        Url::parse(text)
    } else if text.starts_with("//") {
        Url::parse(&format!("{}:{text}", target.scheme()))
    } else {
        Url::parse(&format!("{}://{text}", target.scheme()))
    }
    .ok()
}

/// Compare places, retaining credentials, query and fragment. Only one final
/// path slash, percent-escape hex case and an empty query or fragment are immaterial.
pub(crate) fn comparable(url: &Url) -> String {
    let path = url.path().strip_suffix('/').unwrap_or(url.path());
    let query = url
        .query()
        .filter(|q| !q.is_empty())
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let fragment = url
        .fragment()
        .filter(|q| !q.is_empty())
        .map(|q| format!("#{q}"))
        .unwrap_or_default();
    let text = format!(
        "{}://{}:{}@{}{path}{query}{fragment}",
        url.scheme(),
        url.username(),
        url.password().unwrap_or(""),
        &url[url::Position::BeforeHost..url::Position::AfterPort]
    );
    let mut bytes = text.into_bytes();
    for index in 0..bytes.len().saturating_sub(2) {
        if bytes[index] == b'%'
            && bytes[index + 1].is_ascii_hexdigit()
            && bytes[index + 2].is_ascii_hexdigit()
        {
            bytes[index + 1].make_ascii_uppercase();
            bytes[index + 2].make_ascii_uppercase();
        }
    }
    // Only ASCII hex digits were case-changed, so the bytes are still the valid UTF-8 they came from.
    String::from_utf8(bytes)
        .unwrap_or_else(|error| String::from_utf8_lossy(error.as_bytes()).into_owned())
}

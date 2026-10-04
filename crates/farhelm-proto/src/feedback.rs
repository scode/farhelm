//! The feedback submission the UI composes and the helm forwards (SPEC.md
//! "Feedback", SPEC_impl.md "Feedback forwarding").
//!
//! One type and one set of caps for both ends, so the dialog refuses exactly
//! what the helm would refuse and the helm forwards exactly what the dialog
//! showed. The website endpoint that receives it (`website/feedback/
//! handler.js`) repeats the same caps in JavaScript because it is the public
//! boundary; the two definitions must move together.
//!
//! Lengths are counted in Unicode code points (`chars().count()`), and
//! "whitespace" is the Unicode `White_Space` property ([`char::is_whitespace`]),
//! because those are the definitions the JavaScript side can reproduce
//! exactly (`[...text].length`, `/^\p{White_Space}*$/u`). Nothing here trims or
//! rewrites a field: a submission is valid as sent or refused.

use serde::{Deserialize, Serialize};

/// Longest message, in code points.
pub const FEEDBACK_MESSAGE_MAX_CHARS: usize = 4000;
/// Longest optional contact, in code points.
pub const FEEDBACK_CONTACT_MAX_CHARS: usize = 200;
/// Longest version string, in code points. The UI writes this field, not the
/// user, and keeps it within the cap with [`fit_machine_field`].
pub const FEEDBACK_VERSION_MAX_CHARS: usize = 64;
/// Longest operating-system string, in code points; UI-written like the
/// version.
pub const FEEDBACK_OS_MAX_CHARS: usize = 128;

/// Which Farhelm client the feedback was sent from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackSurface {
    /// The native desktop app.
    Desktop,
    /// The web UI in a browser.
    Web,
}

/// Everything one feedback send carries, exactly as the dialog displays it.
///
/// `contact` is `None` when the user left it empty; the dialog sends `None`
/// rather than a blank string so "no contact" has one spelling on the wire.
/// Unknown fields are refused on the way in: the helm re-serializes this
/// type, so an unknown field would be silently dropped and the user would
/// have been shown something that was never sent. A browser tab can run an
/// older or newer UI than its helm, so fields added later must be optional
/// (`Option` with `#[serde(default)]`) to keep older tabs working; a newer
/// tab against an older helm is refused. The endpoint, by contrast, ignores
/// fields it does not know.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedbackSubmission {
    /// What the user wrote. Required, not blank, at most
    /// [`FEEDBACK_MESSAGE_MAX_CHARS`].
    pub message: String,
    /// How to reach the user, if they said.
    pub contact: Option<String>,
    /// The Farhelm version the sidebar shows.
    pub version: String,
    /// Desktop app or web UI.
    pub surface: FeedbackSurface,
    /// The operating system the browser or webview reports.
    pub os: String,
}

impl FeedbackSubmission {
    /// Check every field against its cap, or say in plain words which one
    /// does not fit. The helm answers a refusal with this text and the dialog
    /// shows it, so it names the field the way the dialog does.
    pub fn validate(&self) -> Result<(), String> {
        if is_blank(&self.message) {
            return Err("the message is empty".to_string());
        }
        if self.message.chars().count() > FEEDBACK_MESSAGE_MAX_CHARS {
            return Err(format!(
                "the message is longer than {FEEDBACK_MESSAGE_MAX_CHARS} characters"
            ));
        }
        if let Some(contact) = &self.contact
            && contact.chars().count() > FEEDBACK_CONTACT_MAX_CHARS
        {
            return Err(format!(
                "the contact is longer than {FEEDBACK_CONTACT_MAX_CHARS} characters"
            ));
        }
        if !is_machine_field(&self.version, FEEDBACK_VERSION_MAX_CHARS) {
            return Err("the version is missing, too long, or not plain text".to_string());
        }
        if !is_machine_field(&self.os, FEEDBACK_OS_MAX_CHARS) {
            return Err("the operating system is missing, too long, or not plain text".to_string());
        }
        Ok(())
    }
}

/// Whether text is empty or only whitespace, in the Unicode `White_Space`
/// sense ([`char::is_whitespace`]), which the endpoint reproduces with
/// `/^\p{White_Space}*$/u` rather than JavaScript's `trim`, which disagrees
/// on two characters.
pub fn is_blank(text: &str) -> bool {
    text.chars().all(char::is_whitespace)
}

/// A UI-written field (version, operating system) as the endpoint accepts
/// it: non-empty, within its cap, and free of control characters, which
/// could otherwise add a line that looks like another metadata field in the
/// filed issue.
fn is_machine_field(value: &str, max_chars: usize) -> bool {
    !value.is_empty() && value.chars().count() <= max_chars && !value.chars().any(char::is_control)
}

/// Fit a UI-written field (version, operating system) for the dialog to
/// display and send: control characters become spaces, an over-long value is
/// shortened and ends with `…`, and an empty value becomes `unknown`.
///
/// The user cannot edit these fields, so a value the helm would refuse could
/// never be fixed from the dialog. Fitting before display keeps the
/// displayed value the sent value and always yields a field that validates.
pub fn fit_machine_field(value: &str, max_chars: usize) -> String {
    let plain: String = value
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    if plain.trim().is_empty() {
        return "unknown".to_string();
    }
    if plain.chars().count() <= max_chars {
        return plain;
    }
    let mut shortened: String = plain.chars().take(max_chars.saturating_sub(1)).collect();
    shortened.push('…');
    shortened
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> FeedbackSubmission {
        FeedbackSubmission {
            message: "The sidebar is great.".to_string(),
            contact: None,
            version: "0.24.0".to_string(),
            surface: FeedbackSurface::Desktop,
            os: "macOS".to_string(),
        }
    }

    /// The wire shape is the contract with the website endpoint, which
    /// reads these exact keys and the two surface words; renaming any of them
    /// silently breaks every send until both sides are redeployed.
    #[test]
    fn serializes_to_the_endpoint_contract() {
        let json = serde_json::to_value(FeedbackSubmission {
            contact: Some("me@example.com".to_string()),
            surface: FeedbackSurface::Web,
            ..valid()
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "message": "The sidebar is great.",
                "contact": "me@example.com",
                "version": "0.24.0",
                "surface": "web",
                "os": "macOS",
            })
        );
        assert_eq!(
            serde_json::to_value(valid()).unwrap()["contact"],
            serde_json::Value::Null
        );
    }

    /// An unknown field is refused rather than dropped: the helm forwards
    /// what it parsed, so a dropped field would be content the dialog showed
    /// but never sent.
    #[test]
    fn refuses_unknown_fields() {
        let known = serde_json::json!({
            "message": "m", "contact": null, "version": "v", "surface": "desktop", "os": "o",
        });
        // The same body without the extra field parses, so the refusal below
        // is about the unknown field and nothing else.
        assert!(serde_json::from_value::<FeedbackSubmission>(known.clone()).is_ok());
        let mut extended = known;
        extended["logs"] = serde_json::json!("everything");
        let error = serde_json::from_value::<FeedbackSubmission>(extended).unwrap_err();
        assert!(error.to_string().contains("unknown field"), "{error}");
    }

    /// Each cap is enforced at its boundary, in code points, matching the
    /// endpoint's JavaScript count; a mismatch would let the app send what
    /// the endpoint refuses, which retrying could never fix.
    #[test]
    fn enforces_each_cap_in_code_points() {
        let emoji = "\u{1F600}";
        let at_cap = FeedbackSubmission {
            message: emoji.repeat(FEEDBACK_MESSAGE_MAX_CHARS),
            contact: Some(emoji.repeat(FEEDBACK_CONTACT_MAX_CHARS)),
            version: "v".repeat(FEEDBACK_VERSION_MAX_CHARS),
            os: "o".repeat(FEEDBACK_OS_MAX_CHARS),
            ..valid()
        };
        assert_eq!(at_cap.validate(), Ok(()));
        let over = |edit: fn(&mut FeedbackSubmission)| {
            let mut submission = at_cap.clone();
            edit(&mut submission);
            submission.validate().unwrap_err()
        };
        assert!(over(|s| s.message.push('x')).contains("message"));
        assert!(over(|s| s.contact.as_mut().unwrap().push('x')).contains("contact"));
        assert!(over(|s| s.version.push('x')).contains("version"));
        assert!(over(|s| s.os.push('x')).contains("operating system"));
        assert!(over(|s| s.version.clear()).contains("version"));
        assert!(over(|s| s.os.clear()).contains("operating system"));
        // A control character could fake another metadata line in the issue.
        let mut forged = valid();
        forged.os = "linux\nsurface: web".to_string();
        assert!(forged.validate().unwrap_err().contains("operating system"));
    }

    /// Whitespace is Unicode `White_Space`: U+0085 counts and U+FEFF does
    /// not, which is where JavaScript's own `trim()` would disagree.
    #[test]
    fn blank_means_unicode_white_space_only() {
        for blank in ["", "  \t\n", "\u{85}\u{2028}\u{3000}"] {
            let submission = FeedbackSubmission {
                message: blank.to_string(),
                ..valid()
            };
            assert_eq!(
                submission.validate(),
                Err("the message is empty".to_string())
            );
        }
        let feff = FeedbackSubmission {
            message: "\u{FEFF}".to_string(),
            ..valid()
        };
        assert_eq!(feff.validate(), Ok(()));
    }

    /// UI-written fields are fitted before display, so the user never faces
    /// a refusal they cannot fix by editing.
    #[test]
    fn machine_fields_are_fitted_to_their_cap() {
        assert_eq!(fit_machine_field("", 10), "unknown");
        assert_eq!(fit_machine_field(" \n", 10), "unknown");
        assert_eq!(fit_machine_field("short", 10), "short");
        assert_eq!(fit_machine_field("a\tb\nc", 10), "a b c");
        let fitted = fit_machine_field(&"é".repeat(300), FEEDBACK_OS_MAX_CHARS);
        assert_eq!(fitted.chars().count(), FEEDBACK_OS_MAX_CHARS);
        assert!(fitted.ends_with('…'));
        // Whatever comes in, the fitted value is one the helm accepts.
        for raw in ["", "\u{7}", &"x\n".repeat(500), "macOS 26.1"] {
            let submission = FeedbackSubmission {
                os: fit_machine_field(raw, FEEDBACK_OS_MAX_CHARS),
                version: fit_machine_field(raw, FEEDBACK_VERSION_MAX_CHARS),
                ..valid()
            };
            assert_eq!(submission.validate(), Ok(()), "{raw:?}");
        }
    }
}

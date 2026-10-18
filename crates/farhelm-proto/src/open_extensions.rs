//! The desktop download-opening choice shared by the helm and settings UI.
//!
//! This is an allowlist of final extensions, not a claim that a listed file is
//! safe. The helm checks the saved file again at open time; the UI uses the
//! same parser to warn about types outside the small set it recognizes.

/// An unset preference has this policy, including on upgraded helms.
pub const DEFAULT: &str = "md, txt, json";

const MAX_TEXT_BYTES: usize = 512;
const MAX_ENTRIES: usize = 32;
const MAX_ENTRY_BYTES: usize = 24;

/// A parsed, normalized list in the order the user entered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenExtensions(Vec<String>);

/// Why the settings UI warns about a listed type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WarningKind {
    Safe,
    Html,
    Other,
}

impl OpenExtensions {
    /// Parse a comma-separated list of final extensions for storage or use.
    ///
    /// Empty entries are harmless, including a trailing comma. A dot inside
    /// an entry is refused because a final-extension check can never match it.
    /// The size caps keep the shared preference row and its rendered warnings
    /// bounded even when an authenticated caller sends a large value. They
    /// apply to the normalized form too: a successful save must still parse
    /// when the helm reads the policy for the next hover or click.
    pub fn parse(raw: &str) -> Result<Self, String> {
        if raw.len() > MAX_TEXT_BYTES {
            return Err(format!("open_extensions exceeds {MAX_TEXT_BYTES} bytes"));
        }
        let mut entries = Vec::new();
        for part in raw.split(',') {
            let entry = part.trim().strip_prefix('.').unwrap_or(part.trim());
            if entry.is_empty() {
                continue;
            }
            if entry.len() > MAX_ENTRY_BYTES {
                return Err(format!(
                    "open_extensions entry exceeds {MAX_ENTRY_BYTES} bytes"
                ));
            }
            if !entry
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'_'))
            {
                return Err(format!(
                    "open_extensions entry {entry:?} must use letters, digits, +, - or _ and no inner dot"
                ));
            }
            let entry = entry.to_ascii_lowercase();
            if !entries.contains(&entry) {
                entries.push(entry);
                if entries.len() > MAX_ENTRIES {
                    return Err(format!("open_extensions has more than {MAX_ENTRIES} types"));
                }
            }
        }
        let list = Self(entries);
        if list.normalized().len() > MAX_TEXT_BYTES {
            return Err(format!(
                "normalized open_extensions exceeds {MAX_TEXT_BYTES} bytes"
            ));
        }
        Ok(list)
    }

    /// Read an optional stored choice; invalid stored text admits nothing.
    ///
    /// NULL means no choice has been made and uses the default. An empty
    /// string is a deliberate choice to turn opening off.
    pub fn from_stored(stored: Option<&str>) -> Option<Self> {
        Self::parse(stored.unwrap_or(DEFAULT)).ok()
    }

    /// The stable text stored by the helm and shown when a write succeeds.
    pub fn normalized(&self) -> String {
        self.0.join(", ")
    }

    /// Whether a saved file's final extension is on this list.
    pub fn contains(&self, extension: &str) -> bool {
        self.0
            .iter()
            .any(|listed| extension.eq_ignore_ascii_case(listed))
    }

    /// Each listed type, in input order, for the settings warning display.
    pub fn entries(&self) -> impl Iterator<Item = (&str, WarningKind)> {
        self.0
            .iter()
            .map(|entry| (entry.as_str(), Self::warning_kind(entry)))
    }

    /// Classify warnings without treating an unknown type as forbidden.
    pub fn warning_kind(extension: &str) -> WarningKind {
        match extension {
            "md" | "txt" | "json" | "log" => WarningKind::Safe,
            "html" | "htm" | "xhtml" | "svg" => WarningKind::Html,
            _ => WarningKind::Other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Normalization preserves user order while removing spelling noise that
    /// would otherwise make one extension appear more than once.
    #[test]
    fn parses_and_normalizes_list() {
        let list = OpenExtensions::parse(" .MD, txt, md, JSON, c++, ").unwrap();
        assert_eq!(list.normalized(), "md, txt, json, c++");
        assert!(list.contains("Md"));
        assert!(!list.contains("html"));
    }

    /// A cleared field must not silently restore defaults; invalid stored
    /// values fail closed until someone repairs the shared preference.
    #[test]
    fn distinguishes_unset_empty_and_invalid_storage() {
        assert_eq!(
            OpenExtensions::from_stored(None).unwrap().normalized(),
            DEFAULT
        );
        assert_eq!(
            OpenExtensions::from_stored(Some("")).unwrap().normalized(),
            ""
        );
        assert!(OpenExtensions::from_stored(Some("tar.gz")).is_none());
    }

    /// The warning vocabulary is independent of the open policy: users may
    /// list unfamiliar types, but the settings page must call them out.
    #[test]
    fn classifies_each_listed_type() {
        let list = OpenExtensions::parse("md, log, html, svg, command").unwrap();
        assert_eq!(
            list.entries().collect::<Vec<_>>(),
            vec![
                ("md", WarningKind::Safe),
                ("log", WarningKind::Safe),
                ("html", WarningKind::Html),
                ("svg", WarningKind::Html),
                ("command", WarningKind::Other),
            ]
        );
    }

    /// Final extensions have no inner dot, and bounded text prevents a
    /// preference write from filling the shared row with arbitrary input.
    #[test]
    fn refuses_unmatchable_or_oversized_entries() {
        assert!(OpenExtensions::parse("tar.gz").is_err());
        assert!(OpenExtensions::parse("a/b").is_err());
        assert!(OpenExtensions::parse(&"x".repeat(MAX_ENTRY_BYTES + 1)).is_err());
        assert!(OpenExtensions::parse(&"x".repeat(MAX_TEXT_BYTES + 1)).is_err());
    }

    /// Comma-space normalization may expand a valid raw input; accepting it
    /// would make the saved policy fail closed on its next read.
    #[test]
    fn normalized_size_limit_round_trips() {
        let entries: Vec<_> = (0..21).map(|index| format!("x{index:022}")).collect();
        let within_limit = entries[..20].join(",");
        let list = OpenExtensions::parse(&within_limit).unwrap();
        assert_eq!(OpenExtensions::parse(&list.normalized()), Ok(list));

        let raw = entries.join(",");
        assert!(raw.len() <= MAX_TEXT_BYTES);
        assert!(OpenExtensions::parse(&raw).is_err());
    }
}

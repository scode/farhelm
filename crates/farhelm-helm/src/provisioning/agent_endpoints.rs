//! Merge or clear the recipe's agent endpoints without replacing user settings.
//!
//! These functions return bytes only. The backend writes them through stdin,
//! never argv, and parse errors deliberately omit the input and parser detail:
//! either may contain a token. An incompatible document is refused intact.

use crate::tensorlake::recipes::ResolvedEndpoint;

/// Set only Claude Code's two Farhelm-owned environment keys. Other top-level
/// settings and environment entries survive; a non-object cannot be merged.
pub(super) fn claude(bytes: Option<&[u8]>, endpoint: &ResolvedEndpoint) -> Result<Vec<u8>, String> {
    let mut value = match bytes {
        Some(bytes) => serde_json::from_slice(bytes)
            .map_err(|_| "~/.claude/settings.json is not valid JSON".to_string())?,
        None => serde_json::json!({}),
    };
    let object = value
        .as_object_mut()
        .ok_or("~/.claude/settings.json must be a JSON object")?;
    let env = object.entry("env").or_insert_with(|| serde_json::json!({}));
    let env = env
        .as_object_mut()
        .ok_or("the env in ~/.claude/settings.json must be an object")?;
    env.insert(
        "ANTHROPIC_BASE_URL".into(),
        endpoint.base_url.clone().into(),
    );
    env.insert(
        "ANTHROPIC_AUTH_TOKEN".into(),
        endpoint.token.clone().unwrap_or_default().into(),
    );
    serde_json::to_vec_pretty(&value).map_err(|_| "could not encode Claude Code's settings".into())
}

/// Select a dedicated Codex provider, retaining the rest of the TOML document,
/// including comments. Existing provider tables may contain unrelated keys.
pub(super) fn codex(bytes: Option<&[u8]>, endpoint: &ResolvedEndpoint) -> Result<Vec<u8>, String> {
    let mut doc = codex_document(bytes)?;
    doc["model_provider"] = toml_edit::value("farhelm");
    if !doc.contains_key("model_providers") {
        doc["model_providers"] = toml_edit::Item::Table(toml_edit::Table::new());
    }
    let providers = doc["model_providers"]
        .as_table_like_mut()
        .ok_or("model_providers in ~/.codex/config.toml must be a table")?;
    if !providers.contains_key("farhelm") {
        providers.insert("farhelm", toml_edit::Item::Table(toml_edit::Table::new()));
    }
    let provider = providers
        .get_mut("farhelm")
        .and_then(toml_edit::Item::as_table_like_mut)
        .ok_or("the farhelm provider in ~/.codex/config.toml must be a table")?;
    for (key, value) in [
        ("name", toml_edit::value("Farhelm")),
        ("base_url", toml_edit::value(&endpoint.base_url)),
        ("wire_api", toml_edit::value("responses")),
        ("requires_openai_auth", toml_edit::value(false)),
        (
            "experimental_bearer_token",
            toml_edit::value(endpoint.token.as_deref().unwrap_or_default()),
        ),
    ] {
        provider.insert(key, value);
    }
    Ok(doc.to_string().into_bytes())
}

/// None and Subscription stop selecting Farhelm's API provider while keeping
/// user-owned provider configuration intact. The table remains available for
/// later endpoint use; only its bearer token and our active selection go away.
/// Already clean or absent files require no write, preserving their exact bytes.
pub(super) fn clear_codex(bytes: Option<&[u8]>) -> Result<Option<Vec<u8>>, String> {
    let mut doc = codex_document(bytes)?;
    let mut changed = false;
    let mut comments = String::new();
    if doc.get("model_provider").and_then(toml_edit::Item::as_str) == Some("farhelm") {
        changed |= remove_codex_key(doc.as_table_mut(), "model_provider", &mut comments);
    }
    if let Some(providers) = doc.get_mut("model_providers") {
        let providers = providers
            .as_table_like_mut()
            .ok_or("model_providers in ~/.codex/config.toml must be a table")?;
        if let Some(provider) = providers.get_mut("farhelm") {
            let provider = provider
                .as_table_like_mut()
                .ok_or("the farhelm provider in ~/.codex/config.toml must be a table")?;
            changed |= remove_codex_key(provider, "experimental_bearer_token", &mut comments);
        }
    }
    if !comments.is_empty() {
        // Removed entries own their comments in toml_edit. Retain them at the
        // document's end, including when no neighbouring key survives cleanup.
        comments.push_str(doc.trailing().as_str().unwrap_or_default());
        doc.set_trailing(comments);
    }
    Ok(changed.then(|| doc.to_string().into_bytes()))
}

/// Extract comment decoration before removing an owned setting. Its value is
/// never rendered or copied; inline and leading comments survive independently.
fn remove_codex_key(
    table: &mut dyn toml_edit::TableLike,
    key: &str,
    comments: &mut String,
) -> bool {
    if let Some(prefix) = table
        .key(key)
        .and_then(|key| key.leaf_decor().prefix())
        .and_then(|raw| raw.as_str())
    {
        comments.push_str(prefix);
        if !prefix.is_empty() && !prefix.ends_with('\n') {
            comments.push('\n');
        }
    }
    if let Some(suffix) = table
        .get(key)
        .and_then(toml_edit::Item::as_value)
        .and_then(|value| value.decor().suffix())
        .and_then(|raw| raw.as_str())
        && suffix.trim_start().starts_with('#')
    {
        comments.push_str(suffix);
        comments.push('\n');
    }
    table.remove(key).is_some()
}

/// Endpoint writes and cleanup share parsing and fixed diagnostics. Returning
/// a parser's detailed error could disclose an existing token from this file.
fn codex_document(bytes: Option<&[u8]>) -> Result<toml_edit::DocumentMut, String> {
    let text = std::str::from_utf8(bytes.unwrap_or_default())
        .map_err(|_| "~/.codex/config.toml is not UTF-8".to_string())?;
    text.parse()
        .map_err(|_| "~/.codex/config.toml is not valid TOML".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cleanup removes only the owned selection and bearer token. It must not
    /// delete a provider table a user has extended or rewrite clean files.
    #[test]
    fn codex_cleanup_keeps_provider_tables_and_unrelated_settings() {
        let text = b"# keep\nmodel_provider = 'farhelm'\nmodel = 'personal'\n[model_providers.farhelm]\nexperimental_bearer_token = 'fictional-private'\nbase_url = 'https://endpoint.test'\ncustom = true\n";
        let cleared = clear_codex(Some(text)).unwrap().unwrap();
        let text = String::from_utf8(cleared).unwrap();
        assert!(text.contains("# keep") && !text.contains("fictional-private"));
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert!(!doc.contains_key("model_provider"));
        assert_eq!(doc["model"].as_str(), Some("personal"));
        assert_eq!(
            doc["model_providers"]["farhelm"]["custom"].as_bool(),
            Some(true)
        );
        assert_eq!(
            doc["model_providers"]["farhelm"]["base_url"].as_str(),
            Some("https://endpoint.test")
        );
        assert!(clear_codex(Some(text.as_bytes())).unwrap().is_none());
        assert!(clear_codex(None).unwrap().is_none());
        assert!(
            clear_codex(Some(b"model_provider = 'personal'\n# untouched\n"))
                .unwrap()
                .is_none()
        );
    }

    /// Recipe setup must preserve personal settings, including other
    /// providers, rather than making an endpoint save a config replacement.
    #[test]
    fn merges_preserve_unrelated_settings() {
        let endpoint = ResolvedEndpoint {
            base_url: "http://127.0.0.1:8080".into(),
            token: Some("secret".into()),
            revision: None,
            incarnation: None,
        };
        let json = claude(
            Some(
                br#"{"permissions":{"allow":[]},"env":{"KEEP":"yes","ANTHROPIC_BASE_URL":"old"}}"#,
            ),
            &endpoint,
        )
        .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(json["env"]["KEEP"], "yes");
        assert_eq!(json["env"]["ANTHROPIC_AUTH_TOKEN"], "secret");
        assert!(json["permissions"].is_object());
        let toml = codex(Some(b"# keep me\nmodel = 'gpt-test'\n[model_providers.other]\nname = 'other'\n[model_providers.farhelm]\ncustom = true\n"), &endpoint).unwrap();
        let text = String::from_utf8(toml).unwrap();
        assert!(text.contains("# keep me"));
        let doc: toml_edit::DocumentMut = text.parse().unwrap();
        assert_eq!(doc["model"].as_str(), Some("gpt-test"));
        assert_eq!(doc["model_provider"].as_str(), Some("farhelm"));
        assert_eq!(
            doc["model_providers"]["other"]["name"].as_str(),
            Some("other")
        );
        assert_eq!(
            doc["model_providers"]["farhelm"]["custom"].as_bool(),
            Some(true)
        );
        assert_eq!(
            doc["model_providers"]["farhelm"]["requires_openai_auth"].as_bool(),
            Some(false)
        );
    }

    /// Malformed or incompatible user files stay untouched, and refusal text
    /// must not repeat the token a parser may have encountered.
    #[test]
    fn incompatible_documents_fail_without_echoing_contents() {
        let endpoint = ResolvedEndpoint {
            base_url: "https://example.test".into(),
            token: None,
            revision: None,
            incarnation: None,
        };
        for bytes in [b"[]".as_slice(), br#"{"env":[]}"#, b"secret invalid"] {
            assert!(
                !claude(Some(bytes), &endpoint)
                    .unwrap_err()
                    .contains("secret")
            );
        }
        for bytes in [b"model_providers = false".as_slice(), b"secret invalid"] {
            assert!(
                !codex(Some(bytes), &endpoint)
                    .unwrap_err()
                    .contains("secret")
            );
            assert!(!clear_codex(Some(bytes)).unwrap_err().contains("secret"));
        }
        assert!(
            clear_codex(Some(b"[model_providers]\nfarhelm = 'secret'\n"))
                .unwrap_err()
                .contains("must be a table")
        );
        assert!(claude(None, &endpoint).is_ok());
        assert!(codex(None, &endpoint).is_ok());
    }
}

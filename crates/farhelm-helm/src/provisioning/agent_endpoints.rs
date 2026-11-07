//! Merge the recipe's agent endpoints without replacing the user's settings.
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
    let text = std::str::from_utf8(bytes.unwrap_or_default())
        .map_err(|_| "~/.codex/config.toml is not UTF-8".to_string())?;
    let mut doc = text
        .parse::<toml_edit::DocumentMut>()
        .map_err(|_| "~/.codex/config.toml is not valid TOML".to_string())?;
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

#[cfg(test)]
mod tests {
    use super::*;

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
        }
        assert!(claude(None, &endpoint).is_ok());
        assert!(codex(None, &endpoint).is_ok());
    }
}

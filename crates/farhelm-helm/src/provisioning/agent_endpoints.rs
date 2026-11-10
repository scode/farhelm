//! Merge or clear the recipe's agent endpoints without replacing user settings.
//!
//! These functions return bytes only. The backend writes them through stdin,
//! never argv, and parse errors deliberately omit the input and parser detail:
//! either may contain a token. An incompatible document is refused intact.

use crate::tensorlake::recipes::ResolvedEndpoint;

/// Select an endpoint without carrying image credentials to the selected URL.
/// Best-effort cleanup removes known conflicting settings; unrelated settings
/// and environment entries survive, including Claude's first-run choices.
pub(super) fn claude(bytes: Option<&[u8]>, endpoint: &ResolvedEndpoint) -> Result<Vec<u8>, String> {
    let mut value = claude_document(bytes)?;
    value
        .as_object_mut()
        .expect("validated Claude settings")
        .remove("apiKeyHelper");
    let env = claude_env(&mut value)?;
    env.remove("CLAUDE_CODE_OAUTH_TOKEN");
    env.remove("ANTHROPIC_CUSTOM_HEADERS");
    env.remove("ANTHROPIC_API_KEY");
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

/// Install a setup-token in Claude's ordinary settings, without completing
/// onboarding. Endpoint credentials and helpers can override a subscription;
/// remove those known conflicts without promising an exhaustive credential scrub.
pub(super) fn claude_subscription(bytes: Option<&[u8]>, token: &str) -> Result<Vec<u8>, String> {
    let mut value = claude_document(bytes)?;
    value
        .as_object_mut()
        .expect("validated Claude settings")
        .remove("apiKeyHelper");
    let env = claude_env(&mut value)?;
    env.remove("ANTHROPIC_BASE_URL");
    env.remove("ANTHROPIC_AUTH_TOKEN");
    env.remove("ANTHROPIC_CUSTOM_HEADERS");
    env.remove("ANTHROPIC_API_KEY");
    env.insert("CLAUDE_CODE_OAUTH_TOKEN".into(), token.into());
    serde_json::to_vec_pretty(&value).map_err(|_| "could not encode Claude Code's settings".into())
}

/// None removes the managed authentication keys without inventing a settings
/// file. A clean file keeps its exact bytes, and malformed input is refused
/// intact with diagnostics that cannot quote its credentials.
pub(super) fn clear_claude(bytes: Option<&[u8]>) -> Result<Option<Vec<u8>>, String> {
    let Some(bytes) = bytes else { return Ok(None) };
    let mut value = claude_document(Some(bytes))?;
    if value.get("env").is_none() {
        return Ok(None);
    }
    let env = claude_env(&mut value)?;
    let mut changed = false;
    for key in [
        "ANTHROPIC_BASE_URL",
        "ANTHROPIC_AUTH_TOKEN",
        "CLAUDE_CODE_OAUTH_TOKEN",
    ] {
        changed |= env.remove(key).is_some();
    }
    changed
        .then(|| {
            serde_json::to_vec_pretty(&value)
                .map_err(|_| "could not encode Claude Code's settings".into())
        })
        .transpose()
}

/// Share Claude's bounded caller-provided document and fixed parse errors
/// across all modes; no parser diagnostic may disclose the saved token.
fn claude_document(bytes: Option<&[u8]>) -> Result<serde_json::Value, String> {
    let value = match bytes {
        Some(bytes) => serde_json::from_slice(bytes)
            .map_err(|_| "~/.claude/settings.json is not valid JSON".to_string())?,
        None => serde_json::json!({}),
    };
    if !value.is_object() {
        return Err("~/.claude/settings.json must be a JSON object".into());
    }
    Ok(value)
}

/// Validate Claude's environment before changing its managed entries.
/// Root credential helpers are handled separately; incompatible shapes are
/// refused without returning partially updated settings.
fn claude_env(
    value: &mut serde_json::Value,
) -> Result<&mut serde_json::Map<String, serde_json::Value>, String> {
    value
        .as_object_mut()
        .expect("validated Claude settings")
        .entry("env")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or_else(|| "the env in ~/.claude/settings.json must be an object".into())
}

/// Replace Farhelm's Codex provider so old credentials cannot follow a new URL.
/// This is best-effort cleanup of mixed credential ownership; root settings,
/// other providers and comments outside the replaced table remain intact.
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
    provider.clear();
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

    /// Switching modes must make the new credential authoritative without
    /// erasing unrelated configuration or inventing onboarding metadata.
    /// None is idempotent and does not create an otherwise absent file.
    #[test]
    fn claude_modes_replace_managed_authentication_without_onboarding() {
        let original = br#"{"permissions":{"allow":["Read"]},"env":{"KEEP":"yes","ANTHROPIC_BASE_URL":"https://old.test","ANTHROPIC_AUTH_TOKEN":"fictional-old"},"theme":"dark"}"#;
        let subscription = claude_subscription(Some(original), "fictional-subscription").unwrap();
        let value: serde_json::Value = serde_json::from_slice(&subscription).unwrap();
        assert_eq!(
            value["env"]["CLAUDE_CODE_OAUTH_TOKEN"],
            "fictional-subscription"
        );
        assert!(value["env"].get("ANTHROPIC_BASE_URL").is_none());
        assert!(value["env"].get("ANTHROPIC_AUTH_TOKEN").is_none());
        assert_eq!(value["env"]["KEEP"], "yes");
        assert_eq!(value["permissions"]["allow"][0], "Read");
        assert_eq!(value["theme"], "dark");
        assert_eq!(
            value.as_object().unwrap().len(),
            3,
            "no first-run fields added"
        );
        let endpoint = ResolvedEndpoint {
            base_url: "https://new.test/v1".into(),
            token: Some("fictional-new".into()),
            revision: None,
            incarnation: None,
        };
        let endpoint = claude(Some(&subscription), &endpoint).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&endpoint).unwrap();
        assert!(value["env"].get("CLAUDE_CODE_OAUTH_TOKEN").is_none());
        assert_eq!(value["env"]["ANTHROPIC_AUTH_TOKEN"], "fictional-new");
        let cleared = clear_claude(Some(&endpoint)).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&cleared).unwrap();
        assert_eq!(value["env"], serde_json::json!({"KEEP":"yes"}));
        assert_eq!(value["theme"], "dark");
        assert!(clear_claude(Some(&cleared)).unwrap().is_none());
        assert!(clear_claude(None).unwrap().is_none());
        assert!(
            clear_claude(Some(br#"{"theme":"dark"}"#))
                .unwrap()
                .is_none()
        );
        for invalid in [
            b"[]".as_slice(),
            br#"{"env":[]}"#,
            b"fictional-private invalid",
        ] {
            assert!(
                !claude_subscription(Some(invalid), "fictional-private")
                    .unwrap_err()
                    .contains("fictional-private")
            );
            assert!(
                !clear_claude(Some(invalid))
                    .unwrap_err()
                    .contains("fictional-private")
            );
        }
    }

    /// Mixed ownership can send gateway headers to another endpoint or let an
    /// image's key/helper override a subscription. Both endpoint token states
    /// and Subscription scrub these settings; None leaves image credentials alone.
    #[test]
    fn claude_delivery_removes_image_credentials_but_none_preserves_them() {
        let original = br#"{"apiKeyHelper":"image-helper","theme":"dark","env":{"KEEP":"yes","ANTHROPIC_BASE_URL":"https://old.test","ANTHROPIC_AUTH_TOKEN":"fictional-old","ANTHROPIC_CUSTOM_HEADERS":"X-Gateway-Key: fictional-old","ANTHROPIC_API_KEY":"fictional-image-key"}}"#;
        let mut outputs = vec![claude_subscription(Some(original), "fictional-login").unwrap()];
        for token in [Some("fictional-new".into()), None] {
            let endpoint = ResolvedEndpoint {
                base_url: "https://new.test".into(),
                token,
                revision: None,
                incarnation: None,
            };
            outputs.push(claude(Some(original), &endpoint).unwrap());
        }
        for output in outputs {
            let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
            assert!(value.get("apiKeyHelper").is_none());
            for key in ["ANTHROPIC_CUSTOM_HEADERS", "ANTHROPIC_API_KEY"] {
                assert!(value["env"].get(key).is_none(), "retained {key}");
            }
            assert_eq!(value["env"]["KEEP"], "yes");
            assert_eq!(value["theme"], "dark");
        }
        let cleared: serde_json::Value =
            serde_json::from_slice(&clear_claude(Some(original)).unwrap().unwrap()).unwrap();
        assert_eq!(cleared["apiKeyHelper"], "image-helper");
        assert_eq!(
            cleared["env"]["ANTHROPIC_CUSTOM_HEADERS"],
            "X-Gateway-Key: fictional-old"
        );
        assert_eq!(cleared["env"]["ANTHROPIC_API_KEY"], "fictional-image-key");
    }

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
        assert!(doc["model_providers"]["farhelm"].get("custom").is_none());
        assert_eq!(
            doc["model_providers"]["farhelm"]["requires_openai_auth"].as_bool(),
            Some(false)
        );
    }

    /// A reused image can carry credentials that outrank the recipe token or
    /// add secret headers. Replace the entire selected provider in each TOML
    /// table form, including nested header tables, without touching its peers.
    #[test]
    fn codex_endpoint_replaces_preexisting_provider_credentials() {
        let endpoint = ResolvedEndpoint {
            base_url: "https://new.test/v1".into(),
            token: Some("fictional-new".into()),
            revision: None,
            incarnation: None,
        };
        for provider in [
            "[model_providers.farhelm]\nenv_key = 'OLD_KEY'\nhttp_headers = { api-key = 'fictional-old' }\nenv_http_headers = { X-Key = 'OLD_HEADER' }\n",
            "[model_providers.farhelm]\nenv_key = 'OLD_KEY'\n[model_providers.farhelm.http_headers]\napi-key = 'fictional-old'\n[model_providers.farhelm.env_http_headers]\nX-Key = 'OLD_HEADER'\n",
            "[model_providers]\nfarhelm = { env_key = 'OLD_KEY', http_headers = { api-key = 'fictional-old' }, env_http_headers = { X-Key = 'OLD_HEADER' } }\n",
        ] {
            let original = format!(
                "# root comment\nmodel = 'personal'\n{provider}\n# peer comment\n[model_providers.other]\nname = 'personal provider'\n"
            );
            let output =
                String::from_utf8(codex(Some(original.as_bytes()), &endpoint).unwrap()).unwrap();
            let doc: toml_edit::DocumentMut = output.parse().unwrap();
            let provider = doc["model_providers"]["farhelm"].as_table_like().unwrap();
            assert_eq!(provider.len(), 5);
            assert_eq!(
                provider.get("base_url").and_then(toml_edit::Item::as_str),
                Some("https://new.test/v1")
            );
            assert_eq!(
                provider
                    .get("experimental_bearer_token")
                    .and_then(toml_edit::Item::as_str),
                Some("fictional-new")
            );
            for key in ["env_key", "http_headers", "env_http_headers"] {
                assert!(!provider.contains_key(key), "retained {key}");
            }
            assert_eq!(doc["model"].as_str(), Some("personal"));
            assert_eq!(
                doc["model_providers"]["other"]["name"].as_str(),
                Some("personal provider")
            );
            assert!(output.contains("# root comment") && output.contains("# peer comment"));
        }
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

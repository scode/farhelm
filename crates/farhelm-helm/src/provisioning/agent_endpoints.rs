//! Compose private launch credentials and remove keys written by the legacy settings delivery.
//!
//! These functions return bytes only. The backend writes them through stdin,
//! never argv, and parse errors deliberately omit the input and parser detail:
//! either may contain a token. An incompatible document is refused intact.

use crate::tensorlake::recipes::ResolvedEndpoint;
use farhelm_supervisor::recipe_credentials::{
    CODEX_ENDPOINT_TOKEN_ENV, CodexEndpoint, RecipeCredentials,
};

/// Compose Claude's part of a new snapshot. Its modes have distinct keys, so a mode change drops every key from
/// the previous mode. A tokenless Claude endpoint retains the empty token value the old settings writer assigned.
pub(super) fn claude_environment(
    endpoint: Option<ResolvedEndpoint>,
    subscription: Option<String>,
) -> RecipeCredentials {
    let env = if let Some(token) = subscription {
        [("CLAUDE_CODE_OAUTH_TOKEN".into(), token)].into()
    } else if let Some(endpoint) = endpoint {
        [
            ("ANTHROPIC_BASE_URL".into(), endpoint.base_url),
            (
                "ANTHROPIC_AUTH_TOKEN".into(),
                endpoint.token.unwrap_or_default(),
            ),
        ]
        .into()
    } else {
        Default::default()
    };
    RecipeCredentials {
        env,
        ..RecipeCredentials::default()
    }
}

/// Add Codex to the shared snapshot. Tokenless endpoints only need the descriptor, which is the part allowed in
/// launch arguments.
pub(super) fn add_codex_endpoint(
    credentials: &mut RecipeCredentials,
    endpoint: Option<ResolvedEndpoint>,
) {
    let Some(endpoint) = endpoint else {
        return;
    };
    credentials.codex_endpoint = Some(CodexEndpoint {
        base_url: endpoint.base_url,
        has_token: endpoint.token.is_some(),
    });
    if let Some(token) = endpoint.token {
        credentials
            .env
            .insert(CODEX_ENDPOINT_TOKEN_ENV.into(), token);
    }
}

/// Remove the three keys an earlier Farhelm could write. Callers establish
/// that Farhelm previously managed this file. A clean file keeps its exact
/// bytes, and malformed input receives a fixed diagnostic.
pub(super) fn clear_claude(bytes: Option<&[u8]>) -> Result<Option<Vec<u8>>, String> {
    let Some(bytes) = bytes else { return Ok(None) };
    let mut value = claude_document(bytes)?;
    let Some(env) = value.get_mut("env") else {
        return Ok(None);
    };
    let env = env
        .as_object_mut()
        .ok_or_else(|| "the env in ~/.claude/settings.json must be an object".to_string())?;
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

/// Reject incompatible settings without displaying the parser's input.
fn claude_document(bytes: &[u8]) -> Result<serde_json::Value, String> {
    let value: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|_| "~/.claude/settings.json is not valid JSON".to_string())?;
    if !value.is_object() {
        return Err("~/.claude/settings.json must be a JSON object".into());
    }
    Ok(value)
}

/// Retire the bearer and provider selection from a Farhelm-stamped file before the first launch-time override.
/// The provider table and other settings remain user-owned.
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

    /// A stamped cleanup removes precisely the old recipe keys. Preserve
    /// unrelated configuration and avoid creating a file for absent settings.
    #[test]
    fn claude_cleanup_preserves_unrelated_and_absent_settings() {
        let original = br#"{"apiKeyHelper":"image-helper","permissions":{"allow":["Read"]},"theme":"dark","env":{"KEEP":"yes","ANTHROPIC_BASE_URL":"https://old.test","ANTHROPIC_AUTH_TOKEN":"fictional-old","CLAUDE_CODE_OAUTH_TOKEN":"fictional-login","ANTHROPIC_CUSTOM_HEADERS":"image-header","ANTHROPIC_API_KEY":"image-key"}}"#;
        let cleared = clear_claude(Some(original)).unwrap().unwrap();
        let value: serde_json::Value = serde_json::from_slice(&cleared).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "apiKeyHelper":"image-helper",
                "permissions":{"allow":["Read"]},
                "theme":"dark",
                "env":{"KEEP":"yes","ANTHROPIC_CUSTOM_HEADERS":"image-header","ANTHROPIC_API_KEY":"image-key"}
            })
        );
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
                !clear_claude(Some(invalid))
                    .unwrap_err()
                    .contains("fictional-private")
            );
        }
    }

    /// Each mode writes a complete snapshot, so a new launch cannot inherit
    /// variables from an earlier delivery mode through the managed file.
    #[test]
    fn claude_environment_carries_only_the_selected_mode() {
        let subscription = claude_environment(None, Some("fictional-login".into()));
        assert_eq!(
            subscription.env,
            [("CLAUDE_CODE_OAUTH_TOKEN".into(), "fictional-login".into())].into()
        );
        for token in [Some("fictional-new"), None] {
            let endpoint = ResolvedEndpoint {
                base_url: "https://new.test".into(),
                token: token.map(str::to_owned),
                revision: None,
                incarnation: None,
            };
            let env = claude_environment(Some(endpoint), None);
            assert_eq!(
                env.env,
                [
                    ("ANTHROPIC_BASE_URL".into(), "https://new.test".into()),
                    (
                        "ANTHROPIC_AUTH_TOKEN".into(),
                        token.unwrap_or_default().into()
                    ),
                ]
                .into()
            );
        }
        assert!(claude_environment(None, None).env.is_empty());
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

    /// A tokenless Codex endpoint still has a descriptor and can coexist with Claude's own settings.
    #[test]
    fn codex_endpoint_coexists_with_claude_in_a_single_snapshot() {
        for token in [None, Some("fictional-codex")] {
            let mut credentials = claude_environment(None, Some("fictional-claude".into()));
            add_codex_endpoint(
                &mut credentials,
                Some(ResolvedEndpoint {
                    base_url: "https://new.test/v1".into(),
                    token: token.map(str::to_owned),
                    revision: None,
                    incarnation: None,
                }),
            );
            let descriptor = credentials.codex_endpoint.unwrap();
            assert_eq!(descriptor.base_url, "https://new.test/v1");
            assert_eq!(descriptor.has_token, token.is_some());
            assert_eq!(
                credentials.env["CLAUDE_CODE_OAUTH_TOKEN"],
                "fictional-claude"
            );
            assert_eq!(
                credentials
                    .env
                    .get(CODEX_ENDPOINT_TOKEN_ENV)
                    .map(String::as_str),
                token
            );
        }
    }

    /// Malformed or incompatible user files stay untouched, and refusal text
    /// must not repeat the token a parser may have encountered.
    #[test]
    fn incompatible_codex_documents_fail_without_echoing_contents() {
        for bytes in [b"model_providers = false".as_slice(), b"secret invalid"] {
            assert!(!clear_codex(Some(bytes)).unwrap_err().contains("secret"));
        }
        assert!(
            clear_codex(Some(b"[model_providers]\nfarhelm = 'secret'\n"))
                .unwrap_err()
                .contains("must be a table")
        );
    }
}

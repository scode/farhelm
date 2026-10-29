//! Container settings retain incomplete editor text until launch or template save.
//!
//! Only paths to credentials belong in this draft. The supervisor reads the file
//! on the selected host; the browser never receives or persists its contents.

use dioxus::prelude::*;
use farhelm_proto::{ContainerConfig, ContainerEngine};

use crate::peer::editable_peer;

/// One reusable container choice, with raw port text preserved for correction.
/// Turning the switch off keeps dormant inputs while omitting the whole choice
/// from the launch. A fresh launcher starts with no remembered container values.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct ContainerDraft {
    pub enabled: bool,
    pub image_file: String,
    pub api_proxy_port: String,
    pub github_token_file: String,
}

/// Field edits apply against the owner's live draft, not a render-time copy.
/// Consecutive browser events can arrive before the next component render.
#[derive(Clone, Debug)]
pub(crate) enum ContainerEdit {
    Enabled(bool),
    ImageFile(String),
    ApiProxyPort(String),
    GithubTokenFile(String),
}

impl ContainerDraft {
    /// Replace exactly one editor value without overwriting a preceding event.
    pub(crate) fn apply(&mut self, edit: ContainerEdit) {
        match edit {
            ContainerEdit::Enabled(value) => self.enabled = value,
            ContainerEdit::ImageFile(value) => self.image_file = value,
            ContainerEdit::ApiProxyPort(value) => self.api_proxy_port = value,
            ContainerEdit::GithubTokenFile(value) => self.github_token_file = value,
        }
    }
}

impl ContainerDraft {
    /// Restore copied launch bytes without rewriting unusual but valid paths.
    pub(crate) fn from_config(config: Option<&ContainerConfig>) -> Self {
        match config {
            Some(config) => Self {
                enabled: true,
                image_file: config.image_file.clone(),
                api_proxy_port: config.api_proxy_port.to_string(),
                github_token_file: config.github_token_file.clone().unwrap_or_default(),
            },
            None => Self::default(),
        }
    }

    /// Validate portable intent only; filesystem checks belong to the host.
    /// Paths are literal: trimming them would change which image or token is used.
    pub(crate) fn config(&self) -> Result<Option<ContainerConfig>, String> {
        if !self.enabled {
            return Ok(None);
        }
        let api_proxy_port = self
            .api_proxy_port
            .parse::<u16>()
            .ok()
            .filter(|p| *p != 0)
            .ok_or_else(|| "Container API proxy port must be between 1 and 65535".to_string())?;
        let config = ContainerConfig {
            engine: ContainerEngine::Podman,
            image_file: self.image_file.clone(),
            api_proxy_port,
            github_token_file: (!self.github_token_file.is_empty())
                .then(|| self.github_token_file.clone()),
        };
        config.validate()?;
        Ok(Some(config))
    }
}

/// Share the same literal-path editors between launch and template editing.
/// The owner admits changes and retires retries; this component holds no state.
#[component]
pub(crate) fn ContainerControls(
    value: ContainerDraft,
    busy: bool,
    #[props(default = true)] available: bool,
    on_change: EventHandler<ContainerEdit>,
) -> Element {
    rsx! {
        div { class: "launch-composer-choice launch-composer-container-choice",
            label { class: "launch-composer-container-toggle",
                "data-tooltip": "Run the agent and its terminal tabs in a rootless Podman container on the session's host",
                input { r#type: "checkbox", aria_label: "Containerized", checked: value.enabled, disabled: busy || !available,
                    onchange: move |event: FormEvent| {
                        if !busy && available {
                            on_change.call(ContainerEdit::Enabled(event.checked()));
                        }
                    },
                }
                "Containerized "
                span { class: "launch-composer-choice-help", "(experimental)" }
            }
            if !available {
                p { class: "launch-composer-choice-help", "Containerized sessions require a managed checkout." }
                if value.enabled {
                    button { r#type: "button", class: "btn btn-neutral", disabled: busy,
                        onclick: move |_| on_change.call(ContainerEdit::Enabled(false)), "Turn off containerization"
                    }
                }
            }
            if value.enabled {
                for (field, label, text, help) in [
                    (0, "Image file", value.image_file.clone(), "Absolute path to an OCI archive on the session's host."),
                    (1, "API proxy port", value.api_proxy_port.clone(), "Host loopback port for your local API proxy; 1–65535."),
                    (2, "GitHub token file", value.github_token_file.clone(), "Optional absolute host path; read again each time an agent or tab starts."),
                ] {
                    label { class: "templates-field launch-composer-container-field",
                        span { class: "launch-composer-section-label", "{label}" }
                        input { r#type: "text", aria_label: label, value: editable_peer(&text),
                            disabled: busy || !available,
                            readonly: text.chars().any(farhelm_proto::text::is_presentation_unsafe),
                            inputmode: if field == 1 { "numeric" } else { "text" },
                            autocomplete: "off", autocorrect: "off", autocapitalize: "none", spellcheck: "false",
                            oninput: move |event: FormEvent| {
                                if busy || !available { return; }
                                on_change.call(match field {
                                    0 => ContainerEdit::ImageFile(event.value()),
                                    1 => ContainerEdit::ApiProxyPort(event.value()),
                                    _ => ContainerEdit::GithubTokenFile(event.value()),
                                });
                            },
                        }
                        span { class: "launch-composer-choice-help", "{help}" }
                    }
                    if text.chars().any(farhelm_proto::text::is_presentation_unsafe) {
                        button { r#type: "button", class: "btn btn-neutral", disabled: busy || !available,
                            onclick: move |_| {
                                on_change.call(match field {
                                    0 => ContainerEdit::ImageFile(String::new()),
                                    1 => ContainerEdit::ApiProxyPort(String::new()),
                                    _ => ContainerEdit::GithubTokenFile(String::new()),
                                });
                            }, "Clear {label}"
                        }
                        p { class: "launch-composer-choice-help", "Invisible formatting is shown escaped. Clear this value before editing." }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Invalid required input must not become an absent container choice. The
    /// launcher uses this distinction to refuse instead of executing on the host.
    #[test]
    fn incomplete_container_choices_never_project_as_host_launches() {
        let mut draft = ContainerDraft {
            enabled: true,
            ..Default::default()
        };
        assert!(draft.config().is_err());
        draft.apply(ContainerEdit::ImageFile("/images/agent archive.oci".into()));
        for invalid in ["", "0", "65536", "-1", "1.5", "port"] {
            draft.apply(ContainerEdit::ApiProxyPort(invalid.into()));
            assert!(draft.config().is_err(), "{invalid:?} must refuse");
            assert_eq!(draft.api_proxy_port, invalid, "retain text for correction");
        }
        draft.apply(ContainerEdit::ApiProxyPort("65535".into()));
        let config = draft.config().unwrap().unwrap();
        assert_eq!(config.image_file, "/images/agent archive.oci");
        assert_eq!(config.api_proxy_port, 65535);
        assert_eq!(config.github_token_file, None);
        draft.apply(ContainerEdit::GithubTokenFile("relative-token".into()));
        assert!(draft.config().is_err());
        draft.apply(ContainerEdit::GithubTokenFile(
            "/tokens/value \u{202e}".into(),
        ));
        assert_eq!(
            draft
                .config()
                .unwrap()
                .unwrap()
                .github_token_file
                .as_deref(),
            Some("/tokens/value \u{202e}")
        );
    }

    /// Copied setups retain literal path bytes, and consecutive field events
    /// compose without relying on a render between them. Fresh drafts stay off.
    #[test]
    fn copied_choices_and_consecutive_edits_preserve_their_values() {
        let mut draft = ContainerDraft::default();
        assert_eq!(draft.config().unwrap(), None);
        draft.apply(ContainerEdit::Enabled(true));
        draft.apply(ContainerEdit::ImageFile("/images/agent".into()));
        draft.apply(ContainerEdit::ApiProxyPort("43210".into()));
        draft.apply(ContainerEdit::GithubTokenFile("/tokens/github".into()));
        let config = draft.config().unwrap().unwrap();
        assert_eq!(ContainerDraft::from_config(Some(&config)), draft);
        draft.apply(ContainerEdit::Enabled(false));
        assert_eq!(draft.config().unwrap(), None);
        draft.apply(ContainerEdit::Enabled(true));
        assert_eq!(draft.config().unwrap(), Some(config));
        assert_eq!(ContainerDraft::default().config().unwrap(), None);
    }
}

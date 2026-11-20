//! The template editor's rows: the name, the "Switches launcher to" choice, and one row per field the template sets,
//! each with its labelled control, its hint, its refusal and its ✕.
//!
//! Ported from the old app's `FieldEditor`, `Choice` and `NameKindControl` (`list/templates.rs` at reference commit
//! `24bc0580`), with their labels, placeholders, hover help and hints word for word. Here's the TLDR of the shapes:
//!
//! - **A field row** (`.templates-set-field`) is the field's controls beside its ✕. A field that does not fit the
//!   template's switch or agent type keeps its controls and says why under them, with the warning edge at its left
//!   (`.templates-incompatible`); a field whose stored text the editor shows escaped says how to replace it.
//! - **A choice** (the old `Choice`) is a segmented group for two to five options and a select for more, counted after
//!   a stored value no option offers is added as "<value> (not offered)", so such a value is shown rather than lost.
//! - **The host** uses the shared [`build_placement_rows`] in authoring mode. Existing host shows the editor's Host
//!   select, whose choices are recorded install identities. New sandbox shows Recipe. New container shows Recipe and
//!   Container host. The editor keeps a stored install identity visible as "Unavailable host (retained)" if no listed
//!   host has it, just as the shared controls retain a missing recipe or container host. The authoring rules (only an
//!   incompatible recipe base disables a recipe; an unusable container host stays choosable, its reason after its
//!   name) are [`PlacementMode::Authoring`]'s. The launcher's note on the recipe's build at the chosen place is not
//!   drawn here (recorded in the parity file).
//!
//! Every edit goes through the editor ([`super::Editor`]), which refuses it while a write is in flight; these rows
//! only draw and hand the user's input on.
//!
//! NOTE: A host's name in the Host select is drawn as its escaped presentation in the select's own words: the select
//! has no host-text labels yet, so a right-to-left name is not isolated from the words around it (recorded in the
//! parity file).

use farhelm_proto::launcher::{
    LaunchCatalogModel, LauncherKind, TemplateDestination, TemplateNameKind,
};
use farhelm_proto::recipes::RecipeView;
use farhelm_proto::ui_channel::HostView;
use farhelm_proto::{LaunchEffort, LaunchHarness, LaunchPermission, LaunchTarget};
use gpui::{
    AnyElement, Context, Div, ElementId, InteractiveElement as _, IntoElement, ParentElement as _,
    Role, SharedString, Stateful, StatefulInteractiveElement as _, Styled as _, WeakEntity, Window,
    div, px,
};
use serde::Serialize;

use super::{Editor, FIELD_GAP, NAME_KIND_HELP, SMALL_LINE_HEIGHT, Text, with_editor};
use crate::components::{
    Button, CustomFlags, HoverHelp, RadioGroup, RadioOption, Select, SelectOption, TextInput, Tier,
    option_heading,
};
use crate::host_text::{self, HostSentence, HostText};
use crate::hosts::identity::display_name;
use crate::launcher::destination_rows::{
    PlacementEdit, PlacementMode, build_placement_rows, destination_choice,
};
use crate::launcher::draft::{DestinationKind, harness_label};
use crate::launcher::model::{ModelOption, OLDER_MODELS, model_options};
use crate::templates::apply::install_identity;
use crate::templates::dialog::TemplatesDialog;
use crate::templates::fields::{Field, effort_options, permission_options};
use crate::theme::{self, Palette, metrics};

/// `.templates-set-field`: the gap between a field's controls and its ✕.
const ROW_GAP: f32 = 8.;
/// `.templates-remove`'s `margin-top`: the ✕'s 34-pixel box sits beside the field's control, below its label (the
/// label's line and the gap under it).
const REMOVE_OFFSET: f32 = LABEL_LINE_HEIGHT + FIELD_GAP;
/// A field label's line: the browser's normal line for the old 13-pixel Inter, about 16 pixels.
const LABEL_LINE_HEIGHT: f32 = 16.;
/// `.templates-incompatible`: the room between the warning edge and the field's controls.
const INCOMPATIBLE_PAD: f32 = 10.;
/// `.templates-model-suggestions`: the gap between suggestions.
const SUGGESTION_GAP: f32 = 4.;
/// The old `Choice`: up to this many options are segments, more a select.
const MAX_SEGMENTS: usize = 5;

/// Everything a row needs to draw: the editor, the dialog its handlers reach it through, and what the store holds.
pub(super) struct Rows<'a> {
    pub(super) editor: &'a Editor,
    pub(super) weak: &'a WeakEntity<TemplatesDialog>,
    pub(super) palette: &'a Palette,
    pub(super) hosts: &'a [HostView],
    /// The pushed recipe catalog, empty before it arrives or while the helm cannot read it.
    pub(super) recipes: &'a [RecipeView],
    /// What the Custom words' input holds now, for the words the block under it shows (empty without the input).
    pub(super) flags: &'a str,
    /// The dialog's model catalog, for the effort choices, the model suggestions and the effort's refusal; `None`
    /// before it is read or when the read failed, which refuses no effort and suggests no model.
    pub(super) catalog: Option<&'a [LaunchCatalogModel]>,
}

impl Rows<'_> {
    /// Whether a write is in flight, which every control below draws busy (or ignores) until it settles.
    fn busy(&self) -> bool {
        self.editor.writing.is_some()
    }

    /// A field's label above its control (`.templates-field`: 13-pixel words on the browser's normal line, 6 pixels
    /// above the control), so the ✕ beside the control lines up with it ([`REMOVE_OFFSET`]).
    fn labelled(&self, label: impl Into<SharedString>, control: impl IntoElement) -> Div {
        div()
            .flex()
            .flex_col()
            .gap(px(FIELD_GAP))
            .min_w_0()
            .child(
                div()
                    .text_size(metrics::TEXT_MD)
                    .line_height(px(LABEL_LINE_HEIGHT))
                    .text_color(theme::color(self.palette.fg_0))
                    .child(label.into()),
            )
            .child(control)
    }

    /// The input of `which`, drawn with its [`TextSpec`](super::TextSpec): its id, label, hover help and face.
    /// Nothing when the input is missing, which the editor never lets happen for a field it draws.
    fn input(&self, which: Text) -> Option<AnyElement> {
        let input = self.editor.inputs.get(&which)?;
        let spec = super::spec(which, &self.editor.draft.fields);
        let field = TextInput::new(
            spec.id,
            &input.state,
            spec.label,
            HoverHelp::tooltip(spec.help),
        );
        let field = if spec.monospace {
            field.monospace()
        } else {
            field
        };
        // Tests find a field by its id; this records nothing outside test builds.
        Some(
            div()
                .debug_selector(move || spec.id.to_owned())
                .child(field)
                .into_any_element(),
        )
    }

    /// The input of `which` under its label, as `.templates-field` drew a labelled input.
    fn labelled_input(&self, which: Text) -> AnyElement {
        let label = super::spec(which, &self.editor.draft.fields).label;
        self.labelled(label, div().children(self.input(which)))
            .into_any_element()
    }

    /// A field that is one labelled input and its hint (Wrapper prefix, Initial prompt, Command, Resume command).
    fn text_row(&self, field: Field, hint_text: &'static str) -> Vec<AnyElement> {
        vec![
            self.labelled_input(Text::Field(field)),
            hint(
                format!("templates-{}-hint", field.key()),
                hint_text,
                self.palette,
            )
            .into_any_element(),
        ]
    }

    /// The template's name (`.templates-name`), shown escaped and read-only when it holds invisible formatting.
    pub(super) fn name_field(&self) -> AnyElement {
        self.labelled_input(Text::TemplateName)
    }

    /// "Switches launcher to": Agent, Command or Don't switch, the launch kind the template stores.
    pub(super) fn switch(&self) -> AnyElement {
        let weak = self.weak.clone();
        let group = [
            (Some(LauncherKind::Agent), "Agent"),
            (Some(LauncherKind::Command), "Command"),
            (None, "Don't switch"),
        ]
        .into_iter()
        .fold(
            RadioGroup::new(
                "templates-switch",
                "Switches launcher to",
                HoverHelp::tooltip(
                    "Switches launcher to: what applying the template does to the launch kind",
                ),
                Role::Group,
            ),
            |group, (kind, label)| {
                group.option(RadioOption::new(
                    label,
                    HoverHelp::tooltip(format!(
                        "Switches launcher to: {label}; incompatible fields stay visible"
                    )),
                    kind,
                ))
            },
        )
        .chosen(Some(self.editor.draft.fields.kind))
        .on_change(move |kind, window, cx| {
            let kind = *kind;
            with_editor(&weak, window, cx, |editor, window, cx| {
                editor.choose_kind(kind, window, cx)
            });
        });
        self.labelled("Switches launcher to", group)
            .into_any_element()
    }

    /// One field the template sets (the old `FieldEditor`): its controls, the read-only hint, its refusal, and its ✕.
    pub(super) fn field(&self, field: Field) -> AnyElement {
        let fields = &self.editor.draft.fields;
        let refusal = field.refusal(fields, self.catalog).map(super::capitalized);
        let mut controls = self.controls(field);
        if field.read_only(fields) {
            controls.push(
                hint(
                    format!("templates-{}-escaped", field.key()),
                    "This field contains characters this editor cannot show directly; they are shown as escapes. \
                     Remove this field with ✕ and add it again to replace its value.",
                    self.palette,
                )
                .into_any_element(),
            );
        }
        if let Some(reason) = refusal.clone() {
            controls.push(
                div()
                    .id(ElementId::Name(
                        format!("templates-{}-refusal", field.key()).into(),
                    ))
                    .debug_selector(move || format!("templates-{}-refusal", field.key()))
                    .role(Role::Alert)
                    .aria_label(reason.clone())
                    .text_size(metrics::TEXT_SM)
                    .text_color(theme::color(self.palette.warn))
                    .child(reason)
                    .into_any_element(),
            );
        }
        let mut column = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(FIELD_GAP))
            .children(controls);
        if refusal.is_some() {
            column = column
                .border_l_2()
                .border_color(theme::color(self.palette.warn))
                .pl(px(INCOMPATIBLE_PAD));
        }
        let noun = field.label().to_ascii_lowercase();
        let remove = Button::new(
            format!("templates-remove-{}", field.key()),
            format!("Remove {noun}"),
            HoverHelp::tooltip(format!("Remove {noun}: this template will leave it as is")),
            Role::Button,
        )
        .content("✕")
        .tier(Tier::FieldRemove)
        .busy(self.busy())
        .on_click(super::on_editor(self.weak, move |editor, window, cx| {
            editor.remove(field, window, cx)
        }));
        div()
            .id(ElementId::Name(
                format!("templates-field-{}", field.key()).into(),
            ))
            .debug_selector(move || format!("templates-field-{}", field.key()))
            .flex()
            .items_start()
            .gap(px(ROW_GAP))
            .child(column)
            .child(div().flex_none().mt(px(REMOVE_OFFSET)).child(remove))
            .into_any_element()
    }

    /// The controls of one field, with their hints, in the old editor's words.
    fn controls(&self, field: Field) -> Vec<AnyElement> {
        let fields = &self.editor.draft.fields;
        let palette = self.palette;
        match field {
            Field::Container => vec![
                hint(
                    "templates-container-retired",
                    "This containerized-session setting is retired. Remove this field to save an ordinary template.",
                    palette,
                )
                .into_any_element(),
            ],
            Field::Wrapper => self.text_row(
                field,
                "An empty input removes the wrapper. The prefix runs through Bash before Farhelm's agent command.",
            ),
            Field::Prompt => self.text_row(
                field,
                "Sent as the agent's first message on every launch from this template. You can still edit it in the \
                 launcher before launching.",
            ),
            Field::Agent => {
                let options = LaunchHarness::ALL
                    .iter()
                    .filter(|agent| fields.kind != Some(LauncherKind::Command) || !agent.is_shell())
                    .map(|&agent| (agent, harness_label(agent).to_owned()))
                    .collect();
                let chosen = fields.agent.unwrap_or(LaunchHarness::ALL[0]);
                vec![self.choice(field, options, chosen, |editor, agent, window, cx| {
                    editor.edit(window, cx, |editor| editor.draft.fields.agent = Some(agent))
                })]
            }
            Field::Model => self.model(),
            Field::Effort => {
                let options = effort_options(fields, self.catalog);
                let chosen = fields.effort.flatten();
                vec![self.choice(
                    field,
                    options,
                    chosen,
                    |editor, effort: Option<LaunchEffort>, window, cx| {
                        editor.edit(window, cx, |editor| {
                            editor.draft.fields.effort = Some(effort)
                        })
                    },
                )]
            }
            Field::Permissions => self.permissions(),
            Field::Yolo => vec![
                self.choice(
                    field,
                    vec![
                        (false, "Keeps safety checks".to_owned()),
                        (true, "YOLO".to_owned()),
                    ],
                    fields.yolo.unwrap_or(false),
                    |editor, yolo, window, cx| {
                        editor.edit(window, cx, |editor| editor.draft.fields.yolo = Some(yolo))
                    },
                ),
                hint(
                    "templates-yolo-hint",
                    "Your statement about this command; Farhelm does not inspect it.",
                    palette,
                )
                .into_any_element(),
            ],
            Field::Command => self.text_row(
                field,
                "Placeholders: {cwd} for the folder and {farhelm_args} for Farhelm's integration arguments; each must \
                 be a whole argument.",
            ),
            Field::Resume => self.text_row(
                field,
                "An empty input sets the default. Placeholders: {cwd}, {farhelm_args}, {conversation}; each must be a \
                 whole argument.",
            ),
            Field::Host => self.host(),
            Field::Destination => self.destination(),
            Field::Name => self.session_name(),
        }
    }

    /// A choice among `options` for `field` (the old `Choice`): segments for two to five options, a select for more,
    /// with a stored value no option offers kept as "<value> (not offered)". `set` makes the user's choice.
    fn choice<V: Clone + PartialEq + Serialize + 'static>(
        &self,
        field: Field,
        mut options: Vec<(V, String)>,
        chosen: V,
        set: fn(&mut Editor, V, &mut Window, &mut Context<TemplatesDialog>),
    ) -> AnyElement {
        if !options.iter().any(|(value, _)| *value == chosen) {
            options.push((chosen.clone(), format!("{} (not offered)", word(&chosen))));
        }
        let label = field.label();
        let help = HoverHelp::tooltip(format!("{label}: the value this template applies"));
        let id = format!("templates-{}", field.key());
        let weak = self.weak.clone();
        let control = if (2..=MAX_SEGMENTS).contains(&options.len()) {
            options
                .into_iter()
                .fold(
                    RadioGroup::new(id, label, help.clone(), Role::Group),
                    |group, (value, text)| {
                        group.option(RadioOption::new(text, help.clone(), value))
                    },
                )
                .chosen(Some(chosen))
                .on_change(move |value, window, cx| {
                    let value = value.clone();
                    with_editor(&weak, window, cx, |editor, window, cx| {
                        set(editor, value, window, cx)
                    });
                })
                .into_any_element()
        } else {
            let chosen_key = options
                .iter()
                .position(|(value, _)| *value == chosen)
                .unwrap_or_default()
                .to_string();
            let entries: Vec<SelectOption> = options
                .iter()
                .enumerate()
                .map(|(index, (_, text))| {
                    SelectOption::new(index.to_string(), text.clone(), help.clone())
                })
                .collect();
            let values: Vec<V> = options.into_iter().map(|(value, _)| value).collect();
            Select::new(id, label, help, Role::ComboBox)
                .options(entries)
                .chosen(chosen_key)
                .on_choose(move |key, window, cx| {
                    let Some(value) = key
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| values.get(index))
                        .cloned()
                    else {
                        return;
                    };
                    with_editor(&weak, window, cx, |editor, window, cx| {
                        set(editor, value, window, cx)
                    });
                })
                .into_any_element()
        };
        self.labelled(label, control).into_any_element()
    }

    /// The model: any text, with the catalog's models for the template's agent type as suggestions under it, each of
    /// which sets the model when clicked, replacing an escaped read-only value too. The suggestions are the agent's
    /// whole list, with earlier members of each family under Older models. Typing in the field does not filter it.
    fn model(&self) -> Vec<AnyElement> {
        let fields = &self.editor.draft.fields;
        let mut controls = vec![self.labelled_input(Text::Field(Field::Model))];
        let (current, older) = model_suggestions(fields.agent, self.catalog);
        for (group_id, label, heading, suggestions) in [
            (
                "templates-model-suggestions",
                "Model suggestions",
                None,
                current,
            ),
            (
                "templates-older-model-suggestions",
                OLDER_MODELS,
                Some("templates-older-models"),
                older,
            ),
        ] {
            if suggestions.is_empty() {
                continue;
            }
            if let Some(heading) = heading {
                controls.push(option_heading(heading, label, self.palette));
            }
            controls.push(
                div()
                    .id(group_id)
                    .role(Role::Group)
                    .aria_label(label)
                    .flex()
                    .flex_wrap()
                    .gap(px(SUGGESTION_GAP))
                    .children(suggestions.into_iter().map(|id| {
                        let shown = HostText::from_host(id.clone());
                        let name = HostSentence::new(format!("templates-suggestion-{id}-name"))
                            .host(&shown)
                            .accessible_name();
                        Button::new(
                            format!("templates-suggestion-{id}"),
                            name,
                            HoverHelp::sentence(
                                HostSentence::new(format!("templates-suggestion-{id}-help"))
                                    .text("Set model to ")
                                    .host(&shown),
                            ),
                            Role::Button,
                        )
                        .tier(Tier::Neutral)
                        .content(host_text::element(&shown))
                        .busy(self.busy())
                        .on_click(super::on_editor(self.weak, move |editor, window, cx| {
                            let id = id.clone();
                            editor.edit(window, cx, |editor| {
                                editor.draft.fields.model = Some(Some(id))
                            })
                        }))
                        .into_any_element()
                    }))
                    .into_any_element(),
            );
        }
        controls.push(
            hint(
                "templates-model-hint",
                "An empty input sets the agent's default model. Any other model text is kept.",
                self.palette,
            )
            .into_any_element(),
        );
        controls
    }

    /// Approvals: the reset and the permissions the agent type offers, and under Custom the words passed in
    /// Farhelm's flags' place, in the Custom flags block the launcher draws too ([`CustomFlags`], the old shared
    /// `CustomPermissionFlags`).
    fn permissions(&self) -> Vec<AnyElement> {
        let fields = &self.editor.draft.fields;
        let options = permission_options(fields.agent);
        if options.is_empty() {
            // A template may carry a permission when its type is changed
            // to a shell. Leave its field removable without offering a mode.
            return vec![div().child("None (shell)").into_any_element()];
        }
        let mut controls = vec![self.choice(
            Field::Permissions,
            options,
            fields.permissions.flatten(),
            |editor, permission: Option<LaunchPermission>, window, cx| {
                editor.choose_permissions(permission, window, cx)
            },
        )];
        if fields.permissions.flatten() != Some(LaunchPermission::Custom) {
            return controls;
        }
        let agent = fields.agent.map_or("the agent", harness_label);
        controls.push(
            CustomFlags::new(
                "templates",
                self.labelled_input(Text::Flags),
                self.flags.to_owned(),
                agent,
            )
            .into_any_element(),
        );
        controls
    }

    /// Draw the shared placement controls with the template's authoring rules. The ordinary Host select stays here
    /// because templates store an install identity rather than the launcher's live host choice.
    fn host(&self) -> Vec<AnyElement> {
        let target = self.editor.draft.fields.launch_target();
        let weak = self.weak.clone();
        let mut controls = build_placement_rows(
            PlacementMode::Authoring,
            target.as_ref(),
            self.recipes,
            self.hosts,
            move |edit, window, cx| {
                with_editor(&weak, window, cx, |editor, window, cx| match edit {
                    PlacementEdit::RunsOn(target) => editor.choose_runs_on(target, window, cx),
                    PlacementEdit::Recipe(recipe) => {
                        editor.edit_target(window, cx, |target| LaunchTarget::Environment {
                            recipe,
                            place: target.place(),
                        })
                    }
                    PlacementEdit::ContainerHost(host) => {
                        editor.edit_target(window, cx, |target| LaunchTarget::Environment {
                            recipe: target.recipe().to_owned(),
                            place: target
                                .place()
                                .on_container_host(host)
                                .unwrap_or(target.place()),
                        })
                    }
                });
            },
            |_, label, control| self.labelled(label, control).into_any_element(),
        );
        if target.is_none() {
            controls.push(self.host_select());
        }
        controls
    }

    /// The Existing host select: the hosts with a recorded install identity, by that identity (SPEC.md: "A template
    /// names an ordinary host by its recorded install identity"), a stored identity no host has kept as retained, and
    /// "Choose a host" while none is chosen, as a newly added field or one that just left an environment starts.
    fn host_select(&self) -> AnyElement {
        let fields = &self.editor.draft.fields;
        let help = HoverHelp::tooltip("Host: the installed machine this template applies");
        let options: Vec<SelectOption> = self
            .hosts
            .iter()
            .filter(|host| !host.is_session_owned)
            .filter_map(|host| {
                let identity = install_identity(host)?;
                let name = HostSentence::new("templates-host-option")
                    .host(&display_name(host))
                    .accessible_name();
                Some(SelectOption::new(identity.to_owned(), name, help.clone()))
            })
            .collect();
        let weak = self.weak.clone();
        let mut select = Select::new("templates-host", "Host", help.clone(), Role::ComboBox)
            .options(options)
            .placeholder("Choose a host")
            .retained("Unavailable host (retained)", help)
            .on_choose(move |identity, window, cx| {
                let identity = identity.to_string();
                with_editor(&weak, window, cx, |editor, window, cx| {
                    // Drawn only on Existing host, where the template names no environment to clear.
                    editor.edit(window, cx, |editor| {
                        editor.draft.fields.host = Some(identity)
                    })
                });
            });
        if let Some(host) = fields.host.as_deref().filter(|host| !host.is_empty()) {
            select = select.chosen(host.to_owned());
        }
        self.labelled("Host", select).into_any_element()
    }

    /// The destination: Folder or Managed checkout (the launcher's destination choice, `destination_choice`),
    /// switching between them keeping the text, and the folder path or repository under it.
    fn destination(&self) -> Vec<AnyElement> {
        let github = matches!(
            self.editor.draft.fields.destination,
            Some(TemplateDestination::Github(_))
        );
        let weak = self.weak.clone();
        let chosen = if github {
            DestinationKind::Checkout
        } else {
            DestinationKind::Folder
        };
        let help =
            HoverHelp::tooltip("Destination type: an existing folder, or a managed checkout");
        let kind = destination_choice(
            "templates-destination-type",
            chosen,
            help,
            move |kind, window, cx| {
                let github = kind == DestinationKind::Checkout;
                with_editor(&weak, window, cx, |editor, window, cx| {
                    editor.edit(window, cx, |editor| {
                        let fields = &mut editor.draft.fields;
                        let text = match fields.destination.take() {
                            Some(
                                TemplateDestination::Folder(text)
                                | TemplateDestination::Github(text),
                            ) => text,
                            None => String::new(),
                        };
                        fields.destination = Some(if github {
                            TemplateDestination::Github(text)
                        } else {
                            TemplateDestination::Folder(text)
                        });
                    })
                });
            },
        );
        let mut controls = vec![
            self.labelled("Destination", kind).into_any_element(),
            // Labelled "Repository" or "Folder path" by its kind (its [`TextSpec`](super::TextSpec)).
            self.labelled_input(Text::Field(Field::Destination)),
        ];
        if github {
            controls.push(
                hint(
                    "templates-checkout-hint",
                    "Each session from this template gets its own managed checkout. Farhelm clones the repository \
                     into a new folder named after the session, under the host's checkout folder. When the last \
                     session using it is deleted, the folder moves to the trash.",
                    self.palette,
                )
                .into_any_element(),
            );
        }
        controls
    }

    /// The session name: its label row with the Base name | Fixed name choice (the old `NameKindControl`; base is the
    /// kind's absence, as stored), the name, and the help explaining numbering and clashes.
    fn session_name(&self) -> Vec<AnyElement> {
        let fixed = self.editor.draft.fields.name_kind == Some(TemplateNameKind::Fixed);
        let weak = self.weak.clone();
        let kind = [
            (None, "Base name"),
            (Some(TemplateNameKind::Fixed), "Fixed name"),
        ]
        .into_iter()
        .fold(
            RadioGroup::new(
                "templates-name-kind",
                "Session name kind",
                HoverHelp::tooltip(format!("Session name kind. {NAME_KIND_HELP}")),
                Role::Group,
            ),
            |group, (value, text)| {
                group.option(RadioOption::new(
                    text,
                    HoverHelp::tooltip(format!("Session name: {text}. {NAME_KIND_HELP}")),
                    value,
                ))
            },
        )
        .chosen(Some(fixed.then_some(TemplateNameKind::Fixed)))
        .on_change(move |kind, window, cx| {
            let kind = *kind;
            with_editor(&weak, window, cx, |editor, window, cx| {
                editor.edit(window, cx, |editor| editor.draft.fields.name_kind = kind)
            });
        });
        let label_row = div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_x(px(12.))
            .gap_y(px(6.))
            .child(
                div()
                    .text_size(metrics::TEXT_MD)
                    .text_color(theme::color(self.palette.fg_0))
                    .child("Session name"),
            )
            .child(div().flex_none().child(kind));
        vec![
            div()
                .flex()
                .flex_col()
                .gap(px(FIELD_GAP))
                .min_w_0()
                .child(label_row)
                .children(self.input(Text::Field(Field::Name)))
                .into_any_element(),
            hint("templates-name-kind-hint", NAME_KIND_HELP, self.palette).into_any_element(),
        ]
    }
}

/// A quiet line of help (`.templates-hint`: 12 pixels at a 1.5 line height in `--fg-1`). It is a labelled node named
/// by its words, since plain text is not in the accessibility tree and a screen reader would otherwise never hear it.
pub(super) fn hint(
    id: impl Into<SharedString>,
    text: impl Into<SharedString>,
    palette: &Palette,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let text: SharedString = text.into();
    div()
        .id(ElementId::Name(id.clone()))
        .debug_selector(move || id.to_string())
        .role(Role::Label)
        .aria_label(text.clone())
        .text_size(metrics::TEXT_SM)
        .line_height(px(SMALL_LINE_HEIGHT))
        .text_color(theme::color(palette.fg_1))
        .child(text)
}

/// The selected agent's current and older model suggestions, in the same order as the launcher.
/// Default and Show all belong to the launcher field, so neither becomes a template suggestion button.
fn model_suggestions(
    agent: Option<LaunchHarness>,
    catalog: Option<&[LaunchCatalogModel]>,
) -> (Vec<String>, Vec<String>) {
    let (Some(agent), Some(catalog)) = (agent, catalog) else {
        return (Vec::new(), Vec::new());
    };
    let mut current = Vec::new();
    let mut older = Vec::new();
    for option in model_options(catalog, Some(agent), "", false) {
        match option {
            ModelOption::Model {
                id, latest: true, ..
            } => current.push(id),
            ModelOption::Model {
                id, latest: false, ..
            } => older.push(id),
            ModelOption::HarnessDefault | ModelOption::ShowAll => {}
        }
    }
    (current, older)
}

/// A stored choice's own word ("xhigh", "smart_approve"), for the "(not offered)" entry that keeps it visible, as the
/// old editor wrote the stored value there.
fn word<V: Serialize>(value: &V) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(word)) => word,
        Ok(serde_json::Value::Null) => "default".to_owned(),
        Ok(other) => other.to_string(),
        Err(_) => String::new(),
    }
}

use super::*;

impl KitterApp {
    pub(super) fn settings_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let language_label = match self.model.library.config.language {
            Language::System => t!("settings.system").to_string(),
            Language::ZhCn => t!("settings.simplified_chinese").to_string(),
            Language::En => t!("settings.english").to_string(),
        };
        let app = cx.entity().downgrade();
        let selected_language = self.model.library.config.language;
        let language_control = Popover::new("language-menu")
            .appearance(false)
            .anchor(Anchor::TopRight)
            .trigger(kit::select_trigger(p, "language-select", language_label).w(px(180.)))
            .content(move |_, _, popover_cx| {
                let mut menu = kit::menu(p).w(px(180.));
                for (index, (language, label)) in [
                    (Language::System, t!("settings.system").to_string()),
                    (
                        Language::ZhCn,
                        t!("settings.simplified_chinese").to_string(),
                    ),
                    (Language::En, t!("settings.english").to_string()),
                ]
                .into_iter()
                .enumerate()
                {
                    let active = selected_language == language;
                    let app = app.clone();
                    menu = menu.child(
                        kit::menu_item(
                            p,
                            ElementId::Name(format!("language-option-{index}").into()),
                            label,
                            active,
                        )
                        .on_click(popover_cx.listener(
                            move |_, _, window, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.set_language(language, window, cx);
                                });
                                cx.emit(DismissEvent);
                            },
                        )),
                    );
                }
                menu
            });
        let preferences_group = kit::card(p).child(
            kit::setting_row(
                p,
                true,
                t!("settings.language").to_string(),
                t!("settings.language_hint").to_string(),
            )
            .child(language_control),
        );
        let appearance_group = kit::card(p).child(
            kit::setting_row(
                p,
                true,
                t!("settings.theme").to_string(),
                t!("settings.appearance_hint").to_string(),
            )
            .child(
                kit::segmented(p)
                    .child(self.small_choice(
                        "theme-system",
                        t!("settings.system").to_string(),
                        self.model.library.config.theme == Theme::System,
                        cx,
                        |this| this.model.library.config.theme = Theme::System,
                    ))
                    .child(self.small_choice(
                        "theme-light",
                        t!("settings.light").to_string(),
                        self.model.library.config.theme == Theme::Light,
                        cx,
                        |this| this.model.library.config.theme = Theme::Light,
                    ))
                    .child(self.small_choice(
                        "theme-dark",
                        t!("settings.dark").to_string(),
                        self.model.library.config.theme == Theme::Dark,
                        cx,
                        |this| this.model.library.config.theme = Theme::Dark,
                    )),
            ),
        );
        let library_group = kit::card(p).child(
            kit::setting_row(
                p,
                true,
                t!("settings.skills_location").to_string(),
                t!("settings.library_hint").to_string(),
            )
            .child(
                div()
                    .w(px(320.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .font_family(MONO)
                            .text_size(px(FS_SMALL))
                            .text_color(p.secondary)
                            .truncate()
                            .child(self.selectable_text(
                                "settings-library-path",
                                400,
                                display_path(&self.model.library.config.library_dir),
                                window,
                                cx,
                            )),
                    )
                    .child(
                        self.bordered_icon_button("browse-library", "icons/folder.svg", cx)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.browse_library(window, cx)),
                            ),
                    ),
            ),
        );

        let registry = crate::agent_plugins::AgentRegistry::global();
        let mut agents_group = kit::card(p);
        let mut rows = 0;
        for agent in registry.agents() {
            rows += 1;
            agents_group = agents_group.child(
                kit::card_row(p, rows == 1)
                    .child(kit::agent_avatar(p, agent))
                    .child(agent.name.clone())
                    .when(
                        !matches!(agent.origin, crate::agent_plugins::PluginOrigin::Builtin),
                        |row| row.child(Self::icon("icons/puzzle.svg", ICON_SM, p.muted)),
                    ),
            );
        }
        let mut listed = std::collections::HashSet::new();
        for diagnostic in registry.diagnostics() {
            let Some(id) = &diagnostic.id else {
                continue;
            };
            if registry.agent(id).is_some() || !listed.insert(id.clone()) {
                continue;
            }
            let message = diagnostic.message.clone();
            rows += 1;
            agents_group = agents_group.child(
                kit::card_row(p, rows == 1)
                    .child(
                        div()
                            .size(px(AVATAR))
                            .flex_none()
                            .rounded_full()
                            .bg(p.avatar)
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(p.text)
                            .child(id.chars().next().unwrap_or_default().to_string()),
                    )
                    .child(id.clone())
                    .child(
                        div()
                            .id(ElementId::Name(format!("agent-error-{id}").into()))
                            .child(Self::icon("icons/triangle-alert.svg", ICON_SM, p.secondary))
                            .tooltip(move |window, cx| {
                                Tooltip::new(message.clone()).build(window, cx)
                            }),
                    ),
            );
        }
        agents_group = agents_group.child(
            kit::card_row(p, rows == 0).child(
                kit::button(
                    p,
                    "open-plugin-directory",
                    kit::ButtonKind::Secondary,
                    kit::Size::Sm,
                    true,
                )
                .child(Self::icon("icons/folder.svg", ICON_SM, p.secondary))
                .child(t!("settings.open_plugin_directory").to_string())
                .on_click(cx.listener(|this, _, _, cx| {
                    let path = crate::agent_plugins::plugin_dir();
                    match std::fs::create_dir_all(&path) {
                        Ok(()) => cx.reveal_path(&path),
                        Err(error) => this.show_notice(
                            t!("settings.plugin_directory_failed", error = error).to_string(),
                            cx,
                        ),
                    }
                })),
            ),
        );

        div()
            .relative()
            .flex_1()
            .h_full()
            .p(px(20.))
            .flex()
            .justify_center()
            .child(
                div()
                    .id("settings-scroll")
                    .w_full()
                    .max_w(px(SETTINGS_CONTENT_MAX_WIDTH))
                    .min_h_0()
                    .overflow_y_scroll()
                    .pb(px(40.))
                    .child(kit::section_title(t!("settings.preferences").to_string()))
                    .child(preferences_group)
                    .child(kit::section_title(t!("settings.appearance").to_string()).mt(px(32.)))
                    .child(appearance_group)
                    .child(kit::section_title(t!("settings.skill_library").to_string()).mt(px(32.)))
                    .child(library_group)
                    .child(kit::section_title(t!("settings.agents").to_string()).mt(px(32.)))
                    .child(agents_group),
            )
            .child(self.window_drag_strip("settings-window-drag", 20., cx))
    }
}

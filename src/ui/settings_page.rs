use super::*;

impl KitterApp {
    pub(super) fn settings_page(&self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let p = self.palette();
        let row = |title: String, description: String| {
            div()
                .min_h(px(60.))
                .px(px(16.))
                .py(px(12.))
                .flex()
                .items_center()
                .gap(px(24.))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(title),
                        )
                        .child(
                            div()
                                .mt(px(2.))
                                .text_size(px(12.))
                                .text_color(p.muted)
                                .child(description),
                        ),
                )
        };
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
            .trigger(
                self.dropdown_button("language-select", language_label, 14.)
                    .w(px(180.)),
            )
            .content(move |_, _, popover_cx| {
                let mut menu = div()
                    .w(px(180.))
                    .p(px(4.))
                    .rounded(px(RADIUS_MENU))
                    .border_1()
                    .border_color(p.border_strong)
                    .bg(p.elevated)
                    .shadow_lg();
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
                        div()
                            .id(ElementId::Name(format!("language-option-{index}").into()))
                            .h(px(29.))
                            .px(px(8.))
                            .rounded(px(RADIUS_CONTROL))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .bg(if active { p.selected } else { rgba(0x00000000) })
                            .hover(move |item| item.bg(p.hover))
                            .font_family(FONT_UI)
                            .text_size(px(13.))
                            .child(label)
                            .on_click(popover_cx.listener(move |_, _, window, cx| {
                                let _ = app.update(cx, |this, cx| {
                                    this.set_language(language, window, cx);
                                });
                                cx.emit(DismissEvent);
                            })),
                    );
                }
                menu
            });
        let preferences_group = div()
            .w_full()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                row(
                    t!("settings.language").to_string(),
                    t!("settings.language_hint").to_string(),
                )
                .child(language_control),
            );
        let appearance_group = div()
            .w_full()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                row(
                    t!("settings.theme").to_string(),
                    t!("settings.appearance_hint").to_string(),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(2.))
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
        let library_group = div()
            .w_full()
            .rounded(px(RADIUS_CARD))
            .border_1()
            .border_color(p.border)
            .bg(p.surface)
            .overflow_hidden()
            .child(
                row(
                    t!("settings.skills_location").to_string(),
                    t!("settings.library_hint").to_string(),
                )
                .child(
                    div()
                        .w(px(320.))
                        .flex()
                        .items_center()
                        .gap(px(7.))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .font_family(MONO)
                                .text_size(px(12.))
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
                            self.bordered_icon_button(
                                "browse-library",
                                "icons/folder.svg",
                                p.text,
                                cx,
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|this, _, window, cx| this.browse_library(window, cx)),
                            ),
                        ),
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
                    .w_full()
                    .max_w(px(SETTINGS_CONTENT_MAX_WIDTH))
                    .pb(px(40.))
                    .child(
                        div()
                            .mb(px(10.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("settings.preferences").to_string()),
                    )
                    .child(preferences_group)
                    .child(
                        div()
                            .mt(px(40.))
                            .mb(px(10.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("settings.appearance").to_string()),
                    )
                    .child(appearance_group)
                    .child(
                        div()
                            .mt(px(40.))
                            .mb(px(10.))
                            .text_size(px(14.))
                            .font_weight(FontWeight::MEDIUM)
                            .child(t!("settings.skill_library").to_string()),
                    )
                    .child(library_group),
            )
            .child(self.window_drag_strip("settings-window-drag", 20., cx))
    }
}

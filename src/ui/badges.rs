use super::*;

impl KitterApp {
    pub(super) fn agent_badges(
        &self,
        popover_id: String,
        targets: &[InstallTarget],
        include_global_only: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let agents = crate::agent_plugins::AgentRegistry::global()
            .agents()
            .iter()
            .filter(|agent| {
                if agent.global_only {
                    include_global_only
                } else {
                    targets.iter().any(|target| agent.supports_target(target))
                }
            })
            .collect::<Vec<_>>();
        self.render_agent_badges(popover_id, agents, cx)
    }

    pub(super) fn effective_agent_badges(
        &self,
        popover_id: String,
        kinds: &[AgentId],
        cx: &mut Context<Self>,
    ) -> Div {
        let agents = crate::agent_plugins::AgentRegistry::global()
            .agents()
            .iter()
            .filter(|agent| kinds.iter().any(|kind| kind.as_str() == agent.id.as_str()))
            .collect::<Vec<_>>();
        self.render_agent_badges(popover_id, agents, cx)
    }

    pub(super) fn render_agent_badges(
        &self,
        popover_id: String,
        agents: Vec<&'static crate::agent_plugins::AgentDef>,
        _cx: &mut Context<Self>,
    ) -> Div {
        let p = self.palette();
        let hidden = agents.len().saturating_sub(5);
        let visible = agents.len().min(5);
        let slots = visible + usize::from(hidden > 0);
        // Avatars overlap by 6px; keep the column width stable for alignment.
        let width = if slots == 0 {
            0.
        } else {
            AVATAR + (slots - 1) as f32 * (AVATAR - 6.)
        };
        let ring = |avatar: Div, index: usize| {
            avatar
                .when(index > 0, |avatar| avatar.ml(px(-6.)))
                .border_1()
                .border_color(p.surface)
        };
        let mut badges = div().flex().items_center();
        for (index, agent) in agents.iter().take(5).enumerate() {
            badges = badges.child(ring(kit::agent_avatar(p, agent), index));
        }
        if hidden > 0 {
            badges = badges.child(
                Popover::new(ElementId::Name(format!("agent-menu-{popover_id}").into()))
                    .appearance(false)
                    .anchor(Anchor::TopRight)
                    .trigger(
                        gpui_base::Button::new(ElementId::Name(
                            format!("agent-more-{popover_id}").into(),
                        ))
                        .accessibility_label(format!("+{hidden}"))
                        .ml(px(-6.))
                        .size(px(AVATAR))
                        .flex_none()
                        .rounded_full()
                        .border_1()
                        .border_color(p.surface)
                        .bg(p.avatar)
                        .flex()
                        .items_center()
                        .justify_center()
                        .font_family(MONO)
                        .text_size(px(10.))
                        .text_color(p.secondary)
                        .hover(move |button| button.text_color(p.text))
                        .child(format!("+{hidden}")),
                    )
                    .content(move |_, _, _| {
                        let mut menu = kit::menu(p)
                            .id("agent-popover-scroll")
                            .w(px(180.))
                            .max_h(px(246.))
                            .overflow_y_scroll();
                        for agent in agents.clone() {
                            menu = menu.child(
                                div()
                                    .h(px(H_SM + 4.))
                                    .px(px(8.))
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .text_size(px(FS_BODY))
                                    .child(kit::agent_avatar(p, agent))
                                    .child(agent.name.clone()),
                            );
                        }
                        menu
                    }),
            );
        }
        div()
            .w(px(width))
            .flex_none()
            .flex()
            .items_center()
            .child(badges)
    }
}

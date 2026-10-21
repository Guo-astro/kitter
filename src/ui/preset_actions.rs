use super::*;
use crate::preset::{AgentsMdState, Preset, PresetStatus, PresetStore, SyncReport};

impl KitterApp {
    pub(super) fn reload_presets(&mut self) {
        // The CLI writes the same store, so pick up its changes on refresh.
        if let Ok(store) = PresetStore::open_in(self.model.library.data_dir()) {
            self.presets_view.store = store;
        }
        self.presets_view.statuses.borrow_mut().clear();
        // Reload AGENTS.md on the next render in case it changed elsewhere.
        *self.presets_view.agents_md_for.borrow_mut() = None;
        let presets = self.presets_view.store.presets();
        if !self
            .presets_view
            .selected
            .as_ref()
            .is_some_and(|id| presets.iter().any(|preset| &preset.id == id))
        {
            self.presets_view.selected = presets.first().map(|preset| preset.id.clone());
        }
    }

    pub(super) fn selected_preset(&self) -> Option<&Preset> {
        let id = self.presets_view.selected.as_deref()?;
        self.presets_view.store.preset(id).ok()
    }

    pub(super) fn select_preset(&mut self, id: String, cx: &mut Context<Self>) {
        self.presets_view.selected = Some(id);
        cx.notify();
    }

    pub(super) fn preset_status(&self, id: &str, project: &Path) -> Option<PresetStatus> {
        let key = (id.to_string(), project.to_path_buf());
        if let Some(status) = self.presets_view.statuses.borrow().get(&key) {
            return status.clone();
        }
        let status = self
            .presets_view
            .store
            .status(id, project, &self.model.skills)
            .ok();
        self.presets_view
            .statuses
            .borrow_mut()
            .insert(key, status.clone());
        status
    }

    pub(super) fn preset_needs_sync(&self, id: &str, project: &Path) -> bool {
        self.preset_status(id, project)
            .is_some_and(|status| status.needs_sync())
    }

    pub(super) fn open_preset_name_dialog(
        &mut self,
        edit: PresetEdit,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value = match &edit {
            PresetEdit::Create => String::new(),
            PresetEdit::Rename(id) => self
                .presets_view
                .store
                .preset(id)
                .map(|preset| preset.name.clone())
                .unwrap_or_default(),
        };
        self.presets_view.name_input.update(cx, |input, cx| {
            input.set_value(value, window, cx);
        });
        self.presets_view.edit = Some(edit);
        self.presets_view.error = None;
        self.open_dialog(DialogKind::PresetName, cx);
        window.focus(&self.presets_view.name_input.focus_handle(cx), cx);
    }

    pub(super) fn commit_preset_name(&mut self, cx: &mut Context<Self>) {
        let Some(edit) = self.presets_view.edit.clone() else {
            return;
        };
        let name = self
            .presets_view
            .name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let result = match &edit {
            PresetEdit::Create => self
                .presets_view
                .store
                .create(&name)
                .map(|preset| Some(preset.id)),
            PresetEdit::Rename(id) => self.presets_view.store.rename(id, &name).map(|_| None),
        };
        match result {
            Ok(created) => {
                if let Some(id) = created {
                    self.presets_view.selected = Some(id);
                    self.presets_view.tab = PresetTab::Skills;
                }
                self.close_dialog(cx);
                self.refresh(cx);
            }
            Err(error) => {
                self.presets_view.error = Some(self.error_message(error));
                self.notify_dialog(cx);
            }
        }
    }

    pub(super) fn open_preset_skills_dialog(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.selected_preset().is_none() {
            return;
        }
        self.presets_view.picked_skills.clear();
        self.presets_view.skill_search.update(cx, |input, cx| {
            input.set_value("", window, cx);
        });
        self.open_dialog(DialogKind::PresetSkills, cx);
        window.focus(&self.presets_view.skill_search.focus_handle(cx), cx);
    }

    pub(super) fn add_picked_preset_skills(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.presets_view.selected.clone() else {
            return;
        };
        let picked = self
            .model
            .skills
            .iter()
            .map(skill_storage_name)
            .filter(|name| self.presets_view.picked_skills.contains(*name))
            .map(str::to_string)
            .collect::<Vec<_>>();
        let result = self.presets_view.store.add_skills(&id, &picked);
        self.close_dialog(cx);
        self.refresh(cx);
        if let Err(error) = result {
            self.show_notice(self.error_message(error), cx);
        }
    }

    /// Removes the skills from the preset when it already holds all of them,
    /// otherwise adds the missing ones.
    pub(super) fn toggle_skills_in_preset(
        &mut self,
        id: &str,
        storage_names: &[String],
        included: bool,
        cx: &mut Context<Self>,
    ) {
        let store = &mut self.presets_view.store;
        let result = if included {
            store.remove_skills(id, storage_names)
        } else {
            store.add_skills(id, storage_names)
        };
        self.refresh(cx);
        if let Err(error) = result {
            self.show_notice(self.error_message(error), cx);
        }
    }

    pub(super) fn remove_preset_skill(
        &mut self,
        id: &str,
        storage_name: &str,
        cx: &mut Context<Self>,
    ) {
        let result = self
            .presets_view
            .store
            .remove_skills(id, &[storage_name.to_string()]);
        self.refresh(cx);
        if let Err(error) = result {
            self.show_notice(self.error_message(error), cx);
        }
    }

    /// Load the selected preset's AGENTS.md into the editor when the
    /// selection changes. Called while rendering the AGENTS.md tab.
    pub(super) fn load_preset_agents_md(&self, id: &str, window: &mut Window, cx: &mut App) {
        if self.presets_view.agents_md_for.borrow().as_deref() == Some(id) {
            return;
        }
        let content = self.presets_view.store.agents_md(id).unwrap_or_default();
        self.presets_view.agents_md.update(cx, |editor, cx| {
            if editor.value().as_ref() != content {
                editor.set_value(content, window, cx);
            }
        });
        *self.presets_view.agents_md_for.borrow_mut() = Some(id.to_string());
    }

    pub(super) fn save_preset_agents_md(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.presets_view.agents_md_for.borrow().clone() else {
            return;
        };
        let content = self.presets_view.agents_md.read(cx).value().to_string();
        if self
            .presets_view
            .store
            .agents_md(&id)
            .is_ok_and(|current| current == content)
        {
            return;
        }
        if let Err(error) = self.presets_view.store.set_agents_md(&id, &content) {
            self.show_notice(self.error_message(error), cx);
            return;
        }
        self.presets_view.statuses.borrow_mut().clear();
        cx.notify();
    }

    pub(super) fn open_preset_install_dialog(
        &mut self,
        id: String,
        project: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let global = match &project {
            Some(path) => dirs::home_dir().as_ref() == Some(path),
            None => self.projects_view.global_project_view,
        };
        if let Some(path) = project.filter(|_| !global) {
            self.projects_view.open_project = Some(path);
        } else if self.projects_view.open_project.is_none() {
            self.projects_view.open_project =
                self.model.library.config.project_paths().first().cloned();
        }
        self.open_install_dialog(window, cx);
        self.install_flow.global = global;
        self.install_flow.preset = Some(id);
    }

    pub(super) fn install_preset(&mut self, plan: project::InstallPlan, cx: &mut Context<Self>) {
        let Some(id) = self.install_flow.preset.clone() else {
            return;
        };
        let Some(project) = (if self.install_flow.global {
            dirs::home_dir()
        } else {
            self.projects_view.open_project.clone()
        }) else {
            self.show_notice(t!("add.project_required").to_string(), cx);
            return;
        };
        let targets = crate::agent_plugins::AgentRegistry::global()
            .install_targets()
            .into_iter()
            .filter(|target| self.install_flow.selected_targets.contains(target))
            .collect::<Vec<_>>();
        let result = self.presets_view.store.install_reviewed(
            &id,
            &project,
            &targets,
            &self.model.skills,
            plan,
        );
        if result.is_ok() && !self.install_flow.global {
            self.model.library.config.touch_project(&project);
            let _ = self.model.library.save();
        }
        self.projects_view.context_estimates.borrow_mut().clear();
        self.close_dialog(cx);
        self.refresh(cx);
        match result {
            Ok(report) => self.show_preset_report(&report, t!("presets.installed"), cx),
            Err(error) => self.show_notice(self.error_message(error), cx),
        }
    }

    /// Sync the preset into the projects. Projects whose AGENTS.md block was
    /// edited locally are only overwritten after confirmation.
    pub(super) fn sync_preset(
        &mut self,
        id: String,
        projects: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let (modified, clean): (Vec<_>, Vec<_>) = projects.into_iter().partition(|project| {
            self.preset_status(&id, project)
                .is_some_and(|status| status.agents_md == AgentsMdState::Modified)
        });
        if !clean.is_empty() {
            self.run_preset_sync(&id, &clean, false, cx);
        }
        if !modified.is_empty() {
            self.presets_view.confirmation = Some(PresetConfirmation::Overwrite(id, modified));
            self.open_dialog(DialogKind::PresetConfirm, cx);
        }
    }

    pub(super) fn run_preset_sync(
        &mut self,
        id: &str,
        projects: &[PathBuf],
        overwrite: bool,
        cx: &mut Context<Self>,
    ) {
        let mut report = SyncReport::default();
        let mut error = None;
        for project in projects {
            match self
                .presets_view
                .store
                .sync(id, project, &self.model.skills, overwrite)
            {
                Ok(partial) => {
                    report.installed += partial.installed;
                    report.removed += partial.removed;
                    report.failures.extend(partial.failures);
                }
                Err(failure) => error = Some(failure),
            }
        }
        self.projects_view.context_estimates.borrow_mut().clear();
        self.refresh(cx);
        match error {
            Some(error) => self.show_notice(self.error_message(error), cx),
            None => self.show_preset_report(&report, t!("presets.synced"), cx),
        }
    }

    pub(super) fn request_preset_uninstall(
        &mut self,
        id: String,
        project: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.presets_view.confirmation = Some(PresetConfirmation::Uninstall(id, project));
        self.open_dialog(DialogKind::PresetConfirm, cx);
    }

    pub(super) fn request_preset_delete(&mut self, id: String, cx: &mut Context<Self>) {
        self.presets_view.confirmation = Some(PresetConfirmation::Delete(id));
        self.open_dialog(DialogKind::PresetConfirm, cx);
    }

    pub(super) fn confirm_preset_action(&mut self, cx: &mut Context<Self>) {
        let Some(confirmation) = self.presets_view.confirmation.take() else {
            return;
        };
        self.close_dialog(cx);
        let skills = self.model.skills.clone();
        let (result, success) = match confirmation {
            PresetConfirmation::Delete(id) => (
                self.presets_view.store.delete(&id, &skills),
                t!("presets.deleted"),
            ),
            PresetConfirmation::Uninstall(id, project) => (
                self.presets_view.store.uninstall(&id, &project, &skills),
                t!("presets.removed"),
            ),
            PresetConfirmation::Overwrite(id, projects) => {
                self.run_preset_sync(&id, &projects, true, cx);
                return;
            }
        };
        self.projects_view.context_estimates.borrow_mut().clear();
        self.refresh(cx);
        match result {
            Ok(report) => self.show_preset_report(&report, success, cx),
            Err(error) => self.show_notice(self.error_message(error), cx),
        }
    }

    fn show_preset_report(
        &mut self,
        report: &SyncReport,
        success: impl Into<String>,
        cx: &mut Context<Self>,
    ) {
        if report.failures.is_empty() {
            self.show_notice(success, cx);
            return;
        }
        for failure in &report.failures {
            eprintln!("Kitter: {failure}");
        }
        let summary = crate::i18n::tn(
            "presets.skills_failed",
            report.failures.len(),
            &[("count", report.failures.len().to_string())],
        );
        let names = report
            .failures
            .iter()
            .map(|failure| failure.skill.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        self.show_notice(format!("{summary}\n{names}"), cx);
    }
}

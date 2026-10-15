use super::KitterApp;

pub(super) use crate::i18n::error_message;

impl KitterApp {
    pub(super) fn source_label(&self, origin: &crate::SkillOrigin) -> String {
        origin.source().label()
    }

    pub(super) fn error_message(&self, error: impl Into<anyhow::Error>) -> String {
        error_message(&error.into())
    }
}

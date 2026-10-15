use crate::config::Language;
use std::fmt;

#[macro_export]
macro_rules! user_error {
    ($key:expr $(, $name:ident = $value:expr)* $(,)?) => {
        $crate::i18n::localized_error($key, &[$((stringify!($name), ($value).to_string())),*])
    };
}

#[macro_export]
macro_rules! user_bail {
    ($($args:tt)*) => {
        return Err($crate::user_error!($($args)*).into())
    };
}

pub fn localized_error(key: &str, args: &[(&str, String)]) -> UserError {
    let message = t!(key);
    let (names, values): (Vec<_>, Vec<_>) = args.iter().cloned().unzip();
    UserError::new(rust_i18n::replace_patterns(&message, &names, &values))
}

pub fn apply_language(language: Language) {
    rust_i18n::set_locale(locale_for(language));
}

pub fn locale_for(language: Language) -> &'static str {
    match language {
        Language::System => locale_for(Language::system()),
        Language::ZhCn => "zh-CN",
        Language::En => "en",
    }
}

pub fn tn(key: &str, count: usize, args: &[(&str, String)]) -> String {
    tn_for_locale(key, count, args, &rust_i18n::locale())
}

pub fn tn_for_locale(key: &str, count: usize, args: &[(&str, String)], locale: &str) -> String {
    let key = format!("{key}.{}", if count == 1 { "one" } else { "other" });
    let message = t!(&key, locale = locale);
    let (names, values): (Vec<_>, Vec<_>) = args.iter().cloned().unzip();
    rust_i18n::replace_patterns(&message, &names, &values)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserError {
    pub message: String,
    pub detail: Option<String>,
}

impl UserError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            detail: None,
        }
    }

    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

impl fmt::Display for UserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for UserError {}

pub fn error_message(error: &anyhow::Error) -> String {
    eprintln!("Kitter: {error:#}");
    for cause in error.chain() {
        if let Some(error) = cause.downcast_ref::<UserError>() {
            if let Some(detail) = &error.detail {
                eprintln!("Kitter: {detail}");
            }
        }
    }
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<UserError>())
        .map(|error| error.message.clone())
        .unwrap_or_else(|| t!("errors.generic").to_string())
}

pub fn error_diagnostic(error: &anyhow::Error) -> String {
    if let Some(error) = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<UserError>())
    {
        return error.detail.as_ref().unwrap_or(&error.message).clone();
    }
    format!("{error:#}")
}

use super::EffectiveSkill;

pub(super) struct CatalogRender {
    pub(super) text: String,
    pub(super) included_count: usize,
}

impl CatalogRender {
    pub(super) fn all(text: String, count: usize) -> Self {
        Self {
            text,
            included_count: count,
        }
    }
}

fn names_only<'a>(names: impl Iterator<Item = &'a str>, budget: usize) -> (String, usize) {
    let mut rows = Vec::new();
    let mut used = 0usize;
    for name in names {
        let row = format!("- {name}");
        let next = row.len() + usize::from(!rows.is_empty());
        if used + next > budget {
            break;
        }
        used += next;
        rows.push(row);
    }
    let count = rows.len();
    (rows.join("\n"), count)
}

fn truncate_chars(value: &str, limit: usize) -> String {
    if value.chars().count() <= limit {
        return value.to_string();
    }
    let mut result = value
        .chars()
        .take(limit.saturating_sub(1))
        .collect::<String>();
    result.push('…');
    result
}

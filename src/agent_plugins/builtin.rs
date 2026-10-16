pub const BUILTIN_PLUGINS: &[(&str, &str)] = &[
    ("codex", include_str!("../../resources/agents/codex.js")),
    (
        "claude-code",
        include_str!("../../resources/agents/claude-code.js"),
    ),
    ("cursor", include_str!("../../resources/agents/cursor.js")),
    (
        "opencode",
        include_str!("../../resources/agents/opencode.js"),
    ),
    ("pi", include_str!("../../resources/agents/pi.js")),
    ("omp", include_str!("../../resources/agents/omp.js")),
    (
        "dimagent",
        include_str!("../../resources/agents/dimagent.js"),
    ),
    ("grok", include_str!("../../resources/agents/grok.js")),
    (
        "openclaw",
        include_str!("../../resources/agents/openclaw.js"),
    ),
    ("hermes", include_str!("../../resources/agents/hermes.js")),
    ("droid", include_str!("../../resources/agents/droid.js")),
    ("amp", include_str!("../../resources/agents/amp.js")),
    (
        "antigravity",
        include_str!("../../resources/agents/antigravity.js"),
    ),
    ("copilot", include_str!("../../resources/agents/copilot.js")),
    ("trae", include_str!("../../resources/agents/trae.js")),
];

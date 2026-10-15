use std::{fmt, sync::Arc};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

const ALIASES: &[(&str, &str)] = &[
    ("claude_code", "claude-code"),
    ("claude", "claude-code"),
    ("open_code", "opencode"),
    ("dim_agent", "dimagent"),
    ("open_claw", "openclaw"),
];

pub(super) fn canonical_id(input: &str) -> &str {
    ALIASES
        .iter()
        .find(|(alias, _)| *alias == input)
        .map_or(input, |(_, id)| *id)
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct AgentId(Arc<str>);

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct InstallTarget(Arc<str>);

impl AgentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AgentId {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for AgentId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for AgentId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(canonical_id(&value)))
    }
}

impl InstallTarget {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn universal() -> Self {
        Self::from("universal")
    }

    pub fn is_universal(&self) -> bool {
        self.as_str() == "universal"
    }
}

impl From<&str> for InstallTarget {
    fn from(value: &str) -> Self {
        Self(Arc::from(value))
    }
}

impl fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for InstallTarget {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for InstallTarget {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        Ok(Self::from(canonical_id(&value)))
    }
}

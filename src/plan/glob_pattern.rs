use serde::{de::Visitor, Deserialize, Deserializer};

#[derive(Debug)]
pub struct GlobPattern(glob::Pattern);

impl GlobPattern {
    pub fn new(pattern: glob::Pattern) -> Self {
        Self(pattern)
    }
    pub fn matches(&self, name: &str) -> bool {
        self.0.matches(name)
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

struct GlobPatternVisitor;
impl<'de> Visitor<'de> for GlobPatternVisitor {
    type Value = GlobPattern;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("failed to parse glob")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        match glob::Pattern::new(v) {
            Ok(glob) => Ok(GlobPattern::new(glob)),
            Err(e) => {
                let msg = format!("failed to parse glob {:?}", e);
                Err(E::custom(msg))
            }
        }
    }
}

impl<'de> Deserialize<'de> for GlobPattern {
    fn deserialize<D>(deserializer: D) -> Result<GlobPattern, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(GlobPatternVisitor)
    }
}

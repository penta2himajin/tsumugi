//! SourceLocation: B 案 (2026-04-23 確定) — sum type with trait for behaviors.
//!
//! - `SourceLocationValue` is the concrete, serializable sum type stored on
//!   `Chunk`. Variants: `File(FileSourceLocation)` and `Custom { schema, payload }`.
//! - The `SourceLocation` trait exposes behaviors (proximity, schema, path,
//!   span). `SourceLocationValue` implements it by dispatching on the variant.
//! - Products can define their own location types and convert to
//!   `SourceLocationValue::Custom` via `TryFrom` / `Into`.

use serde::{Deserialize, Serialize};
use std::ops::Range;

/// Behavior trait for source locations.
///
/// The tuple `(schema, path, span)` is the externally observable identity
/// and `proximity` is the core metric used by `FileProximityScorer`.
pub trait SourceLocation: std::fmt::Debug + Send + Sync {
    /// Schema identifier ("file", "uri", "trpg-session", …). Scorers compare
    /// only within the same schema.
    fn schema(&self) -> &str;

    /// String representation of the path (display / comparison). For
    /// non-file schemas, this is the schema-specific identifier.
    fn path(&self) -> &str;

    /// Byte span within the path, if the location addresses a sub-region.
    fn span(&self) -> Option<Range<usize>> {
        None
    }

    /// Proximity to another location in the `[0.0, 1.0]` range.
    /// Must return `0.0` when `other.schema() != self.schema()`.
    fn proximity(&self, other: &dyn SourceLocation) -> f32;
}

/// Core-shipped variant: file-system path with optional byte span.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSourceLocation {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Range<usize>>,
}

impl FileSourceLocation {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            span: None,
        }
    }

    pub fn with_span(mut self, span: Range<usize>) -> Self {
        self.span = Some(span);
        self
    }

    /// Number of path components separated by `/`.
    fn depth(&self) -> usize {
        self.path.split('/').filter(|s| !s.is_empty()).count()
    }

    /// Length of the common path prefix measured in components.
    fn common_prefix_depth(&self, other: &Self) -> usize {
        self.path
            .split('/')
            .zip(other.path.split('/'))
            .take_while(|(a, b)| a == b)
            .count()
    }
}

impl SourceLocation for FileSourceLocation {
    fn schema(&self) -> &str {
        "file"
    }

    fn path(&self) -> &str {
        &self.path
    }

    fn span(&self) -> Option<Range<usize>> {
        self.span.clone()
    }

    fn proximity(&self, other: &dyn SourceLocation) -> f32 {
        if other.schema() != "file" {
            return 0.0;
        }
        // Same-path shortcut.
        if self.path == other.path() {
            return 1.0;
        }
        // Rebuild a FileSourceLocation view for component comparison.
        let other_path = other.path();
        let common = self
            .path
            .split('/')
            .zip(other_path.split('/'))
            .take_while(|(a, b)| a == b)
            .count();
        let self_depth = self.depth();
        let other_depth = other_path.split('/').filter(|s| !s.is_empty()).count();
        let max_depth = self_depth.max(other_depth).max(1);
        common as f32 / max_depth as f32
    }
}

impl FileSourceLocation {
    /// File-to-file proximity specialized on the concrete type; used when the
    /// caller already knows both sides are files.
    pub fn file_proximity(&self, other: &FileSourceLocation) -> f32 {
        if self.path == other.path {
            return 1.0;
        }
        let common = self.common_prefix_depth(other);
        let max_depth = self.depth().max(other.depth()).max(1);
        common as f32 / max_depth as f32
    }
}

/// Storage form of a source location. Serializes as a tagged enum so external
/// products can round-trip their own location types via `Custom`.
///
/// Note: `Hash` is not derived because `serde_json::Value` does not implement
/// `Hash`. Products that need hashing should build their own key using
/// `schema()` + `path()` + `serde_json::to_string(&value.payload)`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SourceLocationValue {
    File(FileSourceLocation),
    Custom {
        schema: String,
        /// Serialized payload supplied by the product. Proximity is 0.0
        /// across unknown schemas unless a registry is added later (Phase 2).
        payload: serde_json::Value,
    },
}

impl SourceLocationValue {
    pub fn file(path: impl Into<String>) -> Self {
        SourceLocationValue::File(FileSourceLocation::new(path))
    }

    pub fn custom(schema: impl Into<String>, payload: serde_json::Value) -> Self {
        SourceLocationValue::Custom {
            schema: schema.into(),
            payload,
        }
    }
}

impl From<FileSourceLocation> for SourceLocationValue {
    fn from(value: FileSourceLocation) -> Self {
        SourceLocationValue::File(value)
    }
}

impl SourceLocation for SourceLocationValue {
    fn schema(&self) -> &str {
        match self {
            SourceLocationValue::File(_) => "file",
            SourceLocationValue::Custom { schema, .. } => schema,
        }
    }

    fn path(&self) -> &str {
        match self {
            SourceLocationValue::File(f) => f.path(),
            // For Custom, path is the schema — there is no universal path
            // concept across opaque payloads. Products embed their own.
            SourceLocationValue::Custom { schema, .. } => schema,
        }
    }

    fn span(&self) -> Option<Range<usize>> {
        match self {
            SourceLocationValue::File(f) => f.span(),
            SourceLocationValue::Custom { .. } => None,
        }
    }

    fn proximity(&self, other: &dyn SourceLocation) -> f32 {
        match self {
            SourceLocationValue::File(f) => f.proximity(other),
            // Custom variants default to 0.0 across schemas, and 1.0 when
            // the schema + path match. Richer in-schema proximity needs the
            // Phase 2 registry.
            SourceLocationValue::Custom { schema, .. } => {
                if other.schema() != schema {
                    return 0.0;
                }
                if other.path() == schema.as_str() {
                    1.0
                } else {
                    0.0
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_proximity_same_path_is_one() {
        let a = FileSourceLocation::new("src/a/b.rs");
        assert_eq!(a.file_proximity(&a), 1.0);
    }

    #[test]
    fn file_proximity_common_prefix() {
        let a = FileSourceLocation::new("src/a/b.rs");
        let b = FileSourceLocation::new("src/a/c.rs");
        let p = a.file_proximity(&b);
        assert!(p > 0.5 && p < 1.0, "got {p}");
    }

    #[test]
    fn file_proximity_different_trees_is_low() {
        let a = FileSourceLocation::new("src/a/b.rs");
        let b = FileSourceLocation::new("docs/readme.md");
        assert_eq!(a.file_proximity(&b), 0.0);
    }

    #[test]
    fn cross_schema_proximity_is_zero() {
        let a = SourceLocationValue::file("src/x.rs");
        let b = SourceLocationValue::custom("trpg-session", serde_json::json!({"id": 1}));
        assert_eq!(a.proximity(&b), 0.0);
        assert_eq!(b.proximity(&a), 0.0);
    }

    #[test]
    fn source_location_value_file_implements_trait() {
        let v = SourceLocationValue::file("src/x.rs");
        assert_eq!(v.schema(), "file");
        assert_eq!(v.path(), "src/x.rs");
    }

    #[test]
    fn serialize_round_trip() {
        let v = SourceLocationValue::file("src/x.rs");
        let j = serde_json::to_string(&v).unwrap();
        let round: SourceLocationValue = serde_json::from_str(&j).unwrap();
        assert_eq!(v, round);

        let c = SourceLocationValue::custom("trpg-session", serde_json::json!({"id": 42}));
        let j = serde_json::to_string(&c).unwrap();
        let round: SourceLocationValue = serde_json::from_str(&j).unwrap();
        assert_eq!(c, round);
    }
}

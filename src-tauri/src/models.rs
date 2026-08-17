use serde::{Deserialize, Deserializer, Serialize};

/// Shape-compatible with the prototype's mock MODELS array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSummary {
    pub id: String,
    pub name: String,
    pub creator: String,
    pub tags: Vec<String>,
    pub added: String,          // ISO date string
    pub source: String,         // original printables.com URL
    pub files: Vec<FileEntry>,
    pub kinds: Vec<String>,     // deduplicated file extensions
    pub size_mb: f64,
    pub cover_asset_url: Option<String>, // asset://localhost/… path or None
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEntry {
    pub n: String,   // filename (matches prototype convention)
    pub mb: f64,
    pub modified: String,
}

/// Full metadata.json schema v1 — parsed from disk during indexing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataV1 {
    pub schema_version: u32,
    pub source: String,
    pub model_id: String,
    pub slug: String,
    pub name: String,
    pub url: String,
    pub author: String,
    pub tags: Vec<String>,
    pub stats: ModelStats,
    pub description: String,
    pub cloned_at: String,
    pub files: Vec<MetadataFile>,
    pub images: Vec<MetadataImage>,
}

/// Custom deserializer for rating that accepts both string and number formats.
fn deserialize_rating<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let opt: Option<serde_json::Value> = Option::deserialize(deserializer)?;
    match opt {
        None => Ok(None),
        Some(serde_json::Value::Number(n)) => n.as_f64().map(Some).ok_or_else(|| Error::custom("expected number")),
        Some(serde_json::Value::String(s)) => s.parse::<f64>().map(Some).map_err(|e| Error::custom(e.to_string())),
        _ => Err(Error::custom("expected string or number")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelStats {
    pub likes: Option<u64>,
    pub downloads: Option<u64>,
    #[serde(deserialize_with = "deserialize_rating")]
    pub rating: Option<f64>,
    pub published_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataFile {
    pub name: String,
    pub kind: String,
    pub size_bytes: u64,
    pub local_path: String, // relative to model dir: "files/x.3mf"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataImage {
    pub name: String,
    pub kind: String,
    pub local_path: String, // relative to model dir: "images/cover.jpg"
}

/// Aggregated library index returned by `list_models`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryIndex {
    pub models: Vec<ModelSummary>,
    pub totals: LibraryTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryTotals {
    pub models: usize,
    pub files: usize,
    pub bytes_used: u64,
    pub bytes_capacity: u64, // 4 GB default; configurable via prefs
}

/// Prefs stored via tauri-plugin-store (replaces localStorage keys).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPrefs {
    pub theme: String,              // "light" | "dark" | "system"
    pub slicer_key: String,         // "prusa" | "orca"
    pub slicer_executable: Option<String>,
    pub library_folder: Option<String>,
    pub python_path: Option<String>,
}

impl Default for AppPrefs {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            slicer_key: "prusa".into(),
            slicer_executable: None,
            library_folder: None,
            python_path: None,
        }
    }
}

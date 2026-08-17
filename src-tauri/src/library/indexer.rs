//! Library indexer: walks the library folder, parses metadata.json files,
//! and produces a ModelSummary index compatible with the frontend's card renderer.

use crate::models::{FileEntry, LibraryIndex, LibraryTotals, MetadataV1, ModelSummary};
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

/// Walk `library_dir` and return a fully populated LibraryIndex.
pub fn index_library(library_dir: &Path) -> Result<LibraryIndex, String> {
    if !library_dir.exists() {
        return Err(format!("Library directory does not exist: {}", library_dir.display()));
    }

    let mut models: Vec<ModelSummary> = Vec::new();
    let mut total_files: usize = 0;
    let mut total_bytes: u64 = 0;

    for entry in fs::read_dir(library_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let meta_path = entry.path().join("metadata.json");
        if !meta_path.exists() {
            continue;
        }

        match parse_model(&entry.path(), &meta_path) {
            Ok(summary) => {
                total_files += summary.files.len();
                total_bytes += (summary.size_mb * 1_048_576.0) as u64;
                models.push(summary);
            }
            Err(e) => {
                eprintln!("[indexer] skipping {:?}: {e}", entry.path());
            }
        }
    }

    // Sort by added date descending (most recent first)
    models.sort_by(|a, b| b.added.cmp(&a.added));

    let model_count = models.len();
    Ok(LibraryIndex {
        models,
        totals: LibraryTotals {
            models: model_count,
            files: total_files,
            bytes_used: total_bytes,
            bytes_capacity: 4_294_967_296, // 4 GB default
        },
    })
}

fn parse_model(model_dir: &Path, meta_path: &Path) -> Result<ModelSummary, String> {
    let raw = fs::read_to_string(meta_path).map_err(|e| e.to_string())?;
    let meta: MetadataV1 = serde_json::from_str(&raw).map_err(|e| e.to_string())?;

    let files: Vec<FileEntry> = meta
        .files
        .iter()
        .map(|f| {
            let full = model_dir.join(&f.local_path);
            let modified = file_modified_iso(&full);
            FileEntry {
                n: f.name.clone(),
                mb: f.size_bytes as f64 / 1_048_576.0,
                modified,
            }
        })
        .collect();

    let kinds: Vec<String> = meta
        .files
        .iter()
        .map(|f| {
            f.name
                .rsplit('.')
                .next()
                .unwrap_or("unknown")
                .to_lowercase()
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    let size_mb: f64 = meta.files.iter().map(|f| f.size_bytes as f64 / 1_048_576.0).sum();

    // Resolve cover image to asset:// URL if it exists on disk
    let cover_asset_url = meta.images.iter().find(|img| img.kind == "cover").and_then(|img| {
        let full = model_dir.join(&img.local_path);
        if full.exists() {
            Some(format!("asset://localhost/{}", full.display()))
        } else {
            None
        }
    });

    Ok(ModelSummary {
        id: meta.model_id.clone(),
        name: meta.name.clone(),
        creator: meta.author.clone(),
        tags: meta.tags.clone(),
        added: meta.cloned_at.clone(),
        source: meta.url.clone(),
        description: meta.description.clone(),
        files,
        kinds,
        size_mb,
        cover_asset_url,
    })
}

fn file_modified_iso(path: &Path) -> String {
    fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| {
            chrono::DateTime::<chrono::Utc>::from_timestamp(d.as_secs() as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d").to_string())
                .unwrap_or_else(|| "unknown".into())
        })
        .unwrap_or_else(|| "unknown".into())
}

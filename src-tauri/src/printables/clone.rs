//! Pure-Rust model cloning pipeline (replaces `py/printables_clone.py` and `src/python/runner.rs`).

use crate::models::{MetadataFile, MetadataImage, MetadataV1, ModelStats};
use crate::printables::api::{PrintablesClient, RemoteImage};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedModelUrl {
    pub id: String,
    pub slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloneResultSummary {
    pub model_dir: PathBuf,
    pub metadata_path: PathBuf,
    pub metadata: MetadataV1,
}

/// Parses a Printables model URL into `{ id, slug }`.
pub fn parse_model_url(url: &str) -> Option<ParsedModelUrl> {
    let re = Regex::new(
        r"^https?://(?:www\.)?printables\.com/model/(?P<id>\d+)(?:-(?P<slug>[^/?#]+))?",
    )
    .ok()?;
    let caps = re.captures(url.trim())?;
    let id = caps.name("id")?.as_str().to_string();
    let slug = caps
        .name("slug")
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();
    Some(ParsedModelUrl { id, slug })
}

/// Strips characters unsafe for Windows/macOS/Linux filesystems and caps length at 120 chars.
pub fn safe_filename(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            _ => c,
        })
        .collect();
    sanitized.chars().take(120).collect()
}

/// Extracts a safe 1..=4 character lowercase image extension from a URL.
pub fn image_extension_from_url(url: &str) -> String {
    let without_query = url.split('?').next().unwrap_or(url);
    let candidate = without_query.rsplit('.').next().unwrap_or("");
    let clean: String = candidate
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(4)
        .collect::<String>()
        .to_lowercase();
    if clean.is_empty() || clean == "com" {
        "jpg".to_string()
    } else {
        clean
    }
}

/// Executes the full model clone pipeline synchronously into `dest_root`,
/// invoking `emit_progress` with each structured progress record.
pub fn run_clone_pipeline<F>(
    client: &PrintablesClient,
    url: &str,
    dest_root: &Path,
    mut emit_progress: F,
) -> Result<CloneResultSummary, String>
where
    F: FnMut(Value),
{
    let parsed = parse_model_url(url)
        .ok_or_else(|| format!("Not a valid printables.com model URL: {url}"))?;

    let model_id = parsed.id;
    let initial_slug = parsed.slug;

    // Phase 1 – Metadata
    emit_progress(json!({
        "kind": "phase",
        "phase": "metadata",
        "percent": 5,
        "message": "Fetching model metadata…"
    }));

    let meta = client.get_model_metadata(&model_id)?;
    let slug = if !initial_slug.is_empty() {
        initial_slug
    } else if !meta.slug.is_empty() {
        meta.slug.clone()
    } else {
        "model".to_string()
    };

    let model_url = format!("https://www.printables.com/model/{model_id}-{slug}");
    let model_dir = dest_root.join(format!("{model_id}-{slug}"));
    let files_dir = model_dir.join("files");
    let images_dir = model_dir.join("images");

    fs::create_dir_all(&files_dir)
        .map_err(|e| format!("Cannot create files directory: {e}"))?;
    fs::create_dir_all(&images_dir)
        .map_err(|e| format!("Cannot create images directory: {e}"))?;

    emit_progress(json!({
        "kind": "phase",
        "phase": "metadata",
        "percent": 20,
        "message": "Metadata fetched"
    }));

    // Phase 2 – Files
    emit_progress(json!({
        "kind": "phase",
        "phase": "files",
        "percent": 25,
        "message": "Resolving download links…"
    }));

    let files_with_links = client.get_model_files(&model_id)?;
    if files_with_links.is_empty() {
        emit_progress(json!({
            "kind": "phase",
            "phase": "files",
            "percent": 30,
            "message": "No downloadable files found"
        }));
    }

    let total_files = files_with_links.len();
    let mut downloaded_files: Vec<MetadataFile> = Vec::new();

    for (idx, remote_file) in files_with_links.iter().enumerate() {
        let fname = safe_filename(&remote_file.name);
        let dest = files_dir.join(&fname);
        let pct = 30 + (((idx + 1) as f64 / (total_files.max(1) as f64)) * 40.0) as u32;

        emit_progress(json!({
            "kind": "phase",
            "phase": "files",
            "percent": pct,
            "message": format!("Downloading {fname}…"),
            "file": fname,
            "file_index": idx + 1,
            "file_total": total_files,
        }));

        if let Some(ref dl_url) = remote_file.download_url {
            match client.download_file(dl_url, &dest, | _, _ | {}) {
                Ok(size_bytes) => {
                    downloaded_files.push(MetadataFile {
                        name: fname.clone(),
                        kind: remote_file.file_type.clone(),
                        size_bytes,
                        local_path: format!("files/{fname}"),
                    });
                }
                Err(err) => {
                    eprintln!("[clone] Failed to download file {fname}: {err}");
                }
            }
        }
    }

    emit_progress(json!({
        "kind": "phase",
        "phase": "files",
        "percent": 70,
        "message": format!("{} files downloaded", downloaded_files.len())
    }));

    // Phase 3 – Images
    emit_progress(json!({
        "kind": "phase",
        "phase": "images",
        "percent": 75,
        "message": "Fetching image list…"
    }));

    let gallery_images = client.get_model_images(&model_id).unwrap_or_default();
    let mut all_images: Vec<RemoteImage> = Vec::new();
    if let Some(ref cover_url) = meta.main_image_url {
        all_images.push(RemoteImage {
            url: cover_url.clone(),
            kind: "cover".to_string(),
        });
    }
    for img in gallery_images {
        if meta.main_image_url.as_deref() != Some(img.url.as_str()) {
            all_images.push(img);
        }
    }

    let total_images = all_images.len();
    let mut downloaded_images: Vec<MetadataImage> = Vec::new();

    for (idx, img) in all_images.iter().enumerate() {
        let img_kind = if idx == 0 {
            "cover".to_string()
        } else {
            img.kind.clone()
        };
        let ext = image_extension_from_url(&img.url);
        let img_name = format!("image_{idx:02}_{img_kind}.{ext}");
        let img_dest = images_dir.join(&img_name);
        let pct = 75 + (((idx + 1) as f64 / (total_images.max(1) as f64)) * 10.0) as u32;

        emit_progress(json!({
            "kind": "phase",
            "phase": "images",
            "percent": pct,
            "message": format!("Downloading image {} of {}…", idx + 1, total_images)
        }));

        match client.download_file(&img.url, &img_dest, | _, _ | {}) {
            Ok(_) => {
                downloaded_images.push(MetadataImage {
                    name: img_name.clone(),
                    kind: img_kind,
                    local_path: format!("images/{img_name}"),
                });
            }
            Err(err) => {
                eprintln!("[clone] Failed to download image {img_name}: {err}");
            }
        }
    }

    emit_progress(json!({
        "kind": "phase",
        "phase": "images",
        "percent": 85,
        "message": format!("{} images downloaded", downloaded_images.len())
    }));

    // Phase 4 – Finalize metadata.json
    emit_progress(json!({
        "kind": "phase",
        "phase": "finalize",
        "percent": 90,
        "message": "Writing metadata.json…"
    }));

    let cloned_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let metadata = MetadataV1 {
        schema_version: 1,
        source: "printables.com".to_string(),
        model_id: model_id.clone(),
        slug,
        name: meta.name,
        url: model_url,
        author: meta.author,
        tags: meta.tags,
        stats: ModelStats {
            likes: meta.likes,
            downloads: meta.downloads,
            rating: meta.rating,
            published_date: meta.published_date,
        },
        description: meta.description,
        cloned_at,
        files: downloaded_files,
        images: downloaded_images,
    };

    let meta_path = model_dir.join("metadata.json");
    let serialized = serde_json::to_string_pretty(&metadata)
        .map_err(|e| format!("Failed to serialize metadata.json: {e}"))?;
    fs::write(&meta_path, serialized)
        .map_err(|e| format!("Failed to write metadata.json: {e}"))?;

    let abs_model_dir = model_dir.canonicalize().unwrap_or_else(|_| model_dir.clone());
    let abs_meta_path = meta_path.canonicalize().unwrap_or_else(|_| meta_path.clone());

    emit_progress(json!({
        "kind": "done",
        "model_dir": abs_model_dir.display().to_string(),
        "metadata_path": abs_meta_path.display().to_string(),
        "percent": 100
    }));

    Ok(CloneResultSummary {
        model_dir: abs_model_dir,
        metadata_path: abs_meta_path,
        metadata,
    })
}

/// Spawns a background thread to clone `url` into `dest` and streams Tauri events.
pub fn spawn_clone(app: &AppHandle, url: String, dest: String) -> Result<String, String> {
    let client = PrintablesClient::new()?;
    let job_id = uuid::Uuid::new_v4().to_string();
    let app_clone = app.clone();
    let job_id_clone = job_id.clone();

    std::thread::spawn(move || {
        let dest_path = PathBuf::from(&dest);
        let result = run_clone_pipeline(&client, &url, &dest_path, |record| {
            if let Some(kind) = record.get("kind").and_then(|v| v.as_str()) {
                let _ = app_clone.emit(
                    &format!("clone://{}/progress/{}", job_id_clone, kind),
                    &record,
                );
            }
        });

        match result {
            Ok(_) => {
                let _ = app_clone.emit(
                    &format!("clone://{}/done", job_id_clone),
                    &json!({ "job_id": job_id_clone }),
                );
            }
            Err(err_msg) => {
                let _ = app_clone.emit(
                    &format!("clone://{}/error", job_id_clone),
                    &json!({
                        "job_id": job_id_clone,
                        "code": "CLONE_ERROR",
                        "message": err_msg,
                    }),
                );
            }
        }
    });

    Ok(job_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::indexer::index_library;

    #[test]
    fn test_parse_model_url_variants() {
        let p1 = parse_model_url(
            "https://www.printables.com/model/1648230-four-color-multicolor-test-benchy-for-cfs-mmu-copr",
        )
        .unwrap();
        assert_eq!(p1.id, "1648230");
        assert_eq!(
            p1.slug,
            "four-color-multicolor-test-benchy-for-cfs-mmu-copr"
        );

        let p2 = parse_model_url("https://printables.com/model/12345").unwrap();
        assert_eq!(p2.id, "12345");
        assert_eq!(p2.slug, "");

        let p3 = parse_model_url("https://www.printables.com/model/999-test-slug/files?foo=bar").unwrap();
        assert_eq!(p3.id, "999");
        assert_eq!(p3.slug, "test-slug");

        assert!(parse_model_url("https://example.com/model/123").is_none());
    }

    #[test]
    fn test_safe_filename_and_image_ext() {
        assert_eq!(safe_filename("a<b>c:d\"e/f\\g|h?i*j.3mf"), "a_b_c_d_e_f_g_h_i_j.3mf");
        assert_eq!(
            image_extension_from_url("https://media.printables.com/media/prints/123/images/cover.webp?t=123"),
            "webp"
        );
        assert_eq!(
            image_extension_from_url("https://media.printables.com/media/prints/123/images/cover.PNG"),
            "png"
        );
    }

    #[test]
    fn test_end_to_end_clone_and_index_live_model() {
        let temp_dir = std::env::temp_dir().join(format!(
            "printables_offline_rust_clone_test_{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        let client = PrintablesClient::new().unwrap();
        let mut events = Vec::new();
        let url = "https://www.printables.com/model/1648230-four-color-multicolor-test-benchy-for-cfs-mmu-copr";

        let summary = run_clone_pipeline(&client, url, &temp_dir, |ev| {
            events.push(ev);
        })
        .expect("live Rust clone pipeline failed");

        assert!(summary.metadata_path.exists());
        assert_eq!(summary.metadata.model_id, "1648230");
        assert!(!summary.metadata.files.is_empty(), "expected downloaded model files");
        assert!(!summary.metadata.images.is_empty(), "expected downloaded model images");
        assert_eq!(summary.metadata.images[0].kind, "cover");
        assert!(summary.model_dir.join(&summary.metadata.files[0].local_path).exists());
        assert!(summary.model_dir.join(&summary.metadata.images[0].local_path).exists());
        assert!(events.iter().any(|e| e.get("kind").and_then(|v| v.as_str()) == Some("done")));

        // Verify the library indexer indexes the newly cloned model cleanly
        let index = index_library(&temp_dir).expect("index_library failed on cloned model");
        assert_eq!(index.models.len(), 1);
        assert_eq!(index.models[0].id, "1648230");
        assert!(index.models[0].cover_asset_url.is_some());

        let _ = fs::remove_dir_all(&temp_dir);
    }
}

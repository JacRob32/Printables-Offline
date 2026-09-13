//! Collections: user-defined groupings of models, stored as
//! `<library_folder>/collections.json`, entirely separate from each model's
//! own metadata.json so existing model metadata never has to change shape.

use crate::models::{AppPrefs, Collection, CollectionsFile};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;
use uuid::Uuid;

fn collections_path(lib_dir: &Path) -> PathBuf {
    lib_dir.join("collections.json")
}

fn library_dir(prefs: &AppPrefs) -> Result<PathBuf, String> {
    prefs
        .library_folder
        .as_ref()
        .map(PathBuf::from)
        .ok_or_else(|| "Library folder not configured. Set it in Preferences first.".to_string())
}

/// Missing or corrupt collections.json is treated as "no collections yet"
/// rather than an error, so a fresh or older library folder still opens fine.
fn load_collections(lib_dir: &Path) -> CollectionsFile {
    let path = collections_path(lib_dir);
    let Ok(raw) = fs::read_to_string(&path) else {
        return CollectionsFile::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

fn save_collections(lib_dir: &Path, data: &CollectionsFile) -> Result<(), String> {
    let path = collections_path(lib_dir);
    let raw = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(&path, raw).map_err(|e| e.to_string())
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Return all collections defined in the current library folder.
#[tauri::command]
pub fn list_collections(prefs: State<'_, Mutex<AppPrefs>>) -> Result<Vec<Collection>, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib_dir = library_dir(&prefs)?;
    Ok(load_collections(&lib_dir).collections)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCollectionArgs {
    pub name: String,
}

#[tauri::command]
pub fn create_collection(
    args: CreateCollectionArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<Collection, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib_dir = library_dir(&prefs)?;
    let mut data = load_collections(&lib_dir);

    let name = args.name.trim();
    if name.is_empty() {
        return Err("Collection name cannot be empty.".into());
    }

    let now = now_iso();
    let collection = Collection {
        id: Uuid::new_v4().to_string(),
        name: name.to_string(),
        model_ids: Vec::new(),
        created_at: now.clone(),
        updated_at: now,
    };
    data.collections.push(collection.clone());
    save_collections(&lib_dir, &data)?;
    Ok(collection)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameCollectionArgs {
    pub collection_id: String,
    pub name: String,
}

#[tauri::command]
pub fn rename_collection(
    args: RenameCollectionArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<Collection, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib_dir = library_dir(&prefs)?;
    let mut data = load_collections(&lib_dir);

    let name = args.name.trim();
    if name.is_empty() {
        return Err("Collection name cannot be empty.".into());
    }

    let collection = data
        .collections
        .iter_mut()
        .find(|c| c.id == args.collection_id)
        .ok_or("Collection not found.")?;
    collection.name = name.to_string();
    collection.updated_at = now_iso();
    let result = collection.clone();

    save_collections(&lib_dir, &data)?;
    Ok(result)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCollectionArgs {
    pub collection_id: String,
}

#[tauri::command]
pub fn delete_collection(
    args: DeleteCollectionArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<(), String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib_dir = library_dir(&prefs)?;
    let mut data = load_collections(&lib_dir);

    let before = data.collections.len();
    data.collections.retain(|c| c.id != args.collection_id);
    if data.collections.len() == before {
        return Err("Collection not found.".into());
    }

    save_collections(&lib_dir, &data)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionMembershipArgs {
    pub collection_id: String,
    pub model_id: String,
}

/// Add a model to a collection. Idempotent if it's already a member.
#[tauri::command]
pub fn add_model_to_collection(
    args: CollectionMembershipArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<Collection, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib_dir = library_dir(&prefs)?;
    let mut data = load_collections(&lib_dir);

    let collection = data
        .collections
        .iter_mut()
        .find(|c| c.id == args.collection_id)
        .ok_or("Collection not found.")?;
    if !collection.model_ids.iter().any(|id| id == &args.model_id) {
        collection.model_ids.push(args.model_id.clone());
        collection.updated_at = now_iso();
    }
    let result = collection.clone();

    save_collections(&lib_dir, &data)?;
    Ok(result)
}

/// Remove a model from a collection without deleting the model itself.
#[tauri::command]
pub fn remove_model_from_collection(
    args: CollectionMembershipArgs,
    prefs: State<'_, Mutex<AppPrefs>>,
) -> Result<Collection, String> {
    let prefs = prefs.lock().map_err(|e| e.to_string())?;
    let lib_dir = library_dir(&prefs)?;
    let mut data = load_collections(&lib_dir);

    let collection = data
        .collections
        .iter_mut()
        .find(|c| c.id == args.collection_id)
        .ok_or("Collection not found.")?;
    collection.model_ids.retain(|id| id != &args.model_id);
    collection.updated_at = now_iso();
    let result = collection.clone();

    save_collections(&lib_dir, &data)?;
    Ok(result)
}

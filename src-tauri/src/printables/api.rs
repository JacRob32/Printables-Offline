//! Pure-Rust client for the Printables.com GraphQL API and media CDNs.

use regex::Regex;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

const GRAPHQL_ENDPOINT: &str = "https://api.printables.com/graphql/";
const MEDIA_BASE_URL: &str = "https://media.printables.com/";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";

const VALID_ORDERINGS: &[&str] = &["best_match", "popular", "latest", "rating", "makes_count"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMetadata {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub author: String,
    pub tags: Vec<String>,
    pub likes: Option<u64>,
    pub downloads: Option<u64>,
    pub rating: Option<f64>,
    pub published_date: Option<String>,
    pub main_image_url: Option<String>,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub id: String,
    pub name: String,
    pub download_url: Option<String>,
    pub size_bytes: Option<u64>,
    pub file_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteImage {
    pub url: String,
    pub kind: String,
}

pub struct PrintablesClient {
    http: Client,
}

impl PrintablesClient {
    pub fn new() -> Result<Self, String> {
        let http = Client::builder()
            .user_agent(USER_AGENT)
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(15))
            .build()
            .map_err(|e| format!("Failed to initialize HTTP client: {e}"))?;
        Ok(Self { http })
    }

    fn post_graphql(&self, payload: &Value) -> Result<Value, String> {
        let resp = self
            .http
            .post(GRAPHQL_ENDPOINT)
            .json(payload)
            .send()
            .map_err(|e| format!("GraphQL request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(format!("GraphQL HTTP error: {status}"));
        }

        resp.json::<Value>()
            .map_err(|e| format!("Invalid GraphQL JSON response: {e}"))
    }

    /// Searches Printables.com for models using the GraphQL API.
    #[allow(dead_code)]
    pub fn search_models(
        &self,
        search_term: &str,
        limit: usize,
        ordering: &str,
    ) -> Result<Vec<Value>, String> {
        if !VALID_ORDERINGS.contains(&ordering) {
            return Err(format!(
                "Invalid ordering '{ordering}'. Must be one of: {}",
                VALID_ORDERINGS.join(", ")
            ));
        }

        let query = r#"
        query SearchModels($query: String!, $limit: Int, $ordering: SearchChoicesEnum) {
          result: searchPrints2(query: $query, printType: print, limit: $limit, ordering: $ordering) {
            items { ...Model __typename }
          }
        }
        fragment AvatarUser on UserType { id handle publicUsername __typename }
        fragment Model on PrintType {
            id name slug ratingAvg likesCount downloadCount datePublished
            user { ...AvatarUser __typename }
            image { filePath }
            __typename
        }
        "#;

        let payload = json!({
            "operationName": "SearchModels",
            "query": query,
            "variables": {
                "query": search_term,
                "limit": limit,
                "ordering": ordering,
            }
        });

        let data = self.post_graphql(&payload)?;
        Ok(data
            .pointer("/data/result/items")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default())
    }

    /// Fetches model metadata, tags, stats, cover image, and description via GraphQL.
    pub fn get_model_metadata(&self, model_id: &str) -> Result<ModelMetadata, String> {
        let query = r#"
        query ModelTags($id: ID!) {
          model: print(id: $id) {
            id name slug summary description ratingAvg likesCount downloadCount datePublished
            user { publicUsername __typename }
            image { filePath }
            tags { name __typename }
            __typename
          }
        }
        "#;

        let payload = json!({
            "operationName": "ModelTags",
            "query": query,
            "variables": { "id": model_id }
        });

        let data = self.post_graphql(&payload)?;
        let model = data
            .pointer("/data/model")
            .filter(|v| !v.is_null())
            .ok_or_else(|| format!("Model {model_id} not found on Printables.com"))?;

        let name = model
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(model_id)
            .to_string();
        let slug = model
            .get("slug")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let author = model
            .pointer("/user/publicUsername")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let tags = model
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| t.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let likes = model.get("likesCount").and_then(|v| v.as_u64());
        let downloads = model.get("downloadCount").and_then(|v| v.as_u64());
        let rating = match model.get("ratingAvg") {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => s.parse::<f64>().ok(),
            _ => None,
        };
        let published_date = model
            .get("datePublished")
            .and_then(|v| v.as_str())
            .map(String::from);

        let main_image_url = model
            .pointer("/image/filePath")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|fp| format!("{MEDIA_BASE_URL}{}", fp.trim_start_matches('/')));

        let raw_desc = model
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let summary = model
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let cleaned = clean_description_html(raw_desc);
        let description = if !cleaned.is_empty() {
            cleaned
        } else if !summary.trim().is_empty() {
            summary.trim().to_string()
        } else {
            String::new()
        };

        Ok(ModelMetadata {
            id: model_id.to_string(),
            name,
            slug,
            author,
            tags,
            likes,
            downloads,
            rating,
            published_date,
            main_image_url,
            description,
        })
    }

    /// Performs the GetDownloadLink mutation to get a temporary direct download URL.
    pub fn get_real_download_url(
        &self,
        file_id: &str,
        model_id: &str,
        file_type: &str,
    ) -> Result<Option<String>, String> {
        let query = r#"
        mutation GetDownloadLink($id: ID!, $modelId: ID!, $fileType: DownloadFileTypeEnum!, $source: DownloadSourceEnum!) {
          getDownloadLink(id: $id, printId: $modelId, fileType: $fileType, source: $source) {
            ok
            errors {
              ...Error
              __typename
            }
            output {
              link
              count
              ttl
              __typename
            }
            __typename
          }
        }
        fragment Error on ErrorType {
          field
          messages
          __typename
        }
        "#;

        let payload = json!({
            "operationName": "GetDownloadLink",
            "query": query,
            "variables": {
                "id": file_id,
                "modelId": model_id,
                "fileType": file_type,
                "source": "model_detail"
            }
        });

        let data = self.post_graphql(&payload)?;
        let dl = data.pointer("/data/getDownloadLink");
        if let Some(dl) = dl {
            let ok = dl.get("ok").and_then(|v| v.as_bool()).unwrap_or(false);
            if ok {
                let link = dl
                    .pointer("/output/link")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.is_empty())
                    .map(String::from);
                return Ok(link);
            }
        }
        Ok(None)
    }

    /// Fetches the file list and resolves a temporary direct download URL for each file.
    pub fn get_model_files(&self, model_id: &str) -> Result<Vec<RemoteFile>, String> {
        let query = r#"
        query ModelFiles($id: ID!) {
          model: print(id: $id) {
            id
            filesType
            gcodes { ...GcodeDetail __typename }
            stls { ...StlDetail __typename }
            slas { ...SlaDetail __typename }
            otherFiles { ...OtherFileDetail __typename }
            __typename
          }
        }
        fragment GcodeDetail on GCodeType { id created name folder note printer { id name __typename } excludeFromTotalSum printDuration layerHeight nozzleDiameter material { id name __typename } weight fileSize filePreviewPath rawDataPrinter order __typename }
        fragment OtherFileDetail on OtherFileType { id created name folder note fileSize filePreviewPath order __typename }
        fragment SlaDetail on SLAType { id created name folder note expTime firstExpTime printer { id name __typename } printDuration layerHeight usedMaterial fileSize filePreviewPath order __typename }
        fragment StlDetail on STLType { id created name folder note fileSize filePreviewPath order __typename }
        "#;

        let payload = json!({
            "operationName": "ModelFiles",
            "query": query,
            "variables": { "id": model_id }
        });

        let data = self.post_graphql(&payload)?;
        let Some(model_files) = data.pointer("/data/model").filter(|v| !v.is_null()) else {
            return Ok(Vec::new());
        };

        let supported = [("stls", "stl"), ("gcodes", "gcode"), ("slas", "sla"), ("otherFiles", "other")];
        let mut results = Vec::new();

        for (list_name, api_type) in supported {
            if let Some(items) = model_files.get(list_name).and_then(|v| v.as_array()) {
                for item in items {
                    let Some(file_id) = item.get("id").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let Some(file_name) = item.get("name").and_then(|v| v.as_str()) else {
                        continue;
                    };
                    let size_bytes = item.get("fileSize").and_then(|v| v.as_u64());
                    let download_url = self
                        .get_real_download_url(file_id, model_id, api_type)
                        .ok()
                        .flatten();

                    results.push(RemoteFile {
                        id: file_id.to_string(),
                        name: file_name.to_string(),
                        download_url,
                        size_bytes,
                        file_type: api_type.to_string(),
                    });

                    std::thread::sleep(Duration::from_millis(250));
                }
            }
        }

        Ok(results)
    }

    /// Fetches all gallery image URLs for a model using the GraphQL API.
    pub fn get_model_images(&self, model_id: &str) -> Result<Vec<RemoteImage>, String> {
        let query = r#"
        query ModelImages($id: ID!) {
          model: print(id: $id) {
            id
            images {
              filePath
              __typename
            }
            __typename
          }
        }
        "#;

        let payload = json!({
            "operationName": "ModelImages",
            "query": query,
            "variables": { "id": model_id }
        });

        let data = self.post_graphql(&payload)?;
        let Some(images_arr) = data
            .pointer("/data/model/images")
            .and_then(|v| v.as_array())
        else {
            return Ok(Vec::new());
        };

        let mut images = Vec::new();
        for img in images_arr {
            if let Some(fp) = img
                .get("filePath")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                images.push(RemoteImage {
                    url: format!("{MEDIA_BASE_URL}{}", fp.trim_start_matches('/')),
                    kind: "photo".to_string(),
                });
            }
        }

        Ok(images)
    }

    /// Downloads a URL to `dest`, reporting `(bytes_done, bytes_total)` via callback.
    pub fn download_file<F>(&self, url: &str, dest: &Path, mut on_progress: F) -> Result<u64, String>
    where
        F: FnMut(u64, u64),
    {
        let mut resp = self
            .http
            .get(url)
            .send()
            .map_err(|e| format!("Download request failed for {url}: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            return Err(format!("Download HTTP {status} for {url}"));
        }

        let total = resp.content_length().unwrap_or(0);
        let mut file = File::create(dest)
            .map_err(|e| format!("Failed to create {}: {e}", dest.display()))?;

        let mut buf = [0u8; 8192];
        let mut done: u64 = 0;
        loop {
            let n = resp
                .read(&mut buf)
                .map_err(|e| format!("Error reading stream from {url}: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n])
                .map_err(|e| format!("Error writing {}: {e}", dest.display()))?;
            done += n as u64;
            on_progress(done, total);
        }

        Ok(done)
    }
}

/// Converts Printables HTML description into the clean markdown-like text expected by the UI.
pub fn clean_description_html(html: &str) -> String {
    if html.trim().is_empty() {
        return String::new();
    }

    let mut text = html.replace("\r\n", "\n");

    // Convert headings <h1>..<h4> into "\n## Heading\n"
    if let Ok(re_h) = Regex::new(r"(?is)<h[1-4][^>]*>(.*?)</h[1-4]>") {
        text = re_h
            .replace_all(&text, |caps: &regex::Captures| {
                let inner = strip_tags(&caps[1]);
                let decoded = decode_html_entities(inner.trim());
                if decoded.is_empty() {
                    String::new()
                } else {
                    format!("\n## {}\n", decoded)
                }
            })
            .into_owned();
    }

    // Convert <a ... href="url" ...>label</a> into "[label](url)"
    if let Ok(re_a) = Regex::new(r#"(?is)<a\b[^>]*?\bhref\s*=\s*["']([^"']*)["'][^>]*>(.*?)</a>"#)
    {
        text = re_a
            .replace_all(&text, |caps: &regex::Captures| {
                let href = caps[1].trim();
                let label = decode_html_entities(strip_tags(&caps[2]).trim());
                if label.is_empty() {
                    String::new()
                } else if href.is_empty() {
                    label
                } else {
                    format!("[{label}]({href})")
                }
            })
            .into_owned();
    }

    // Line breaks and block endings
    if let Ok(re_br) = Regex::new(r"(?i)<br\s*/?>") {
        text = re_br.replace_all(&text, "\n").into_owned();
    }
    if let Ok(re_li_open) = Regex::new(r"(?i)<li[^>]*>") {
        text = re_li_open.replace_all(&text, "\n• ").into_owned();
    }
    if let Ok(re_block_close) = Regex::new(r"(?i)</(p|div|li|ul|ol|blockquote|tr)>") {
        text = re_block_close.replace_all(&text, "\n").into_owned();
    }

    let stripped = strip_tags(&text);
    let decoded = decode_html_entities(&stripped);

    let mut lines: Vec<String> = Vec::new();
    for raw_line in decoded.lines() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        lines.push(trimmed.to_string());
    }

    lines.join("\n")
}

fn strip_tags(input: &str) -> String {
    if let Ok(re_tags) = Regex::new(r"(?s)<[^>]+>") {
        re_tags.replace_all(input, "").into_owned()
    } else {
        input.to_string()
    }
}

fn decode_html_entities(input: &str) -> String {
    let mut s = input
        .replace("&nbsp;", " ")
        .replace("&#160;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&ndash;", "–")
        .replace("&mdash;", "—");

    if let Ok(re_num) = Regex::new(r"&#(\d+);") {
        s = re_num
            .replace_all(&s, |caps: &regex::Captures| {
                caps[1]
                    .parse::<u32>()
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
                    .unwrap_or_default()
            })
            .into_owned();
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_description_html_headings_links_and_entities() {
        let raw = r#"
            <div class="user-inserted">
                <h3>Print Settings</h3>
                <p>Use 0.2mm layer height&nbsp;and <a href="https://www.printables.com">Printables</a> profile.</p>
                <p>Supports: No<br/>Infill: 15 &amp; 20%</p>
            </div>
        "#;
        let cleaned = clean_description_html(raw);
        assert_eq!(
            cleaned,
            "## Print Settings\nUse 0.2mm layer height and [Printables](https://www.printables.com) profile.\nSupports: No\nInfill: 15 & 20%"
        );
    }

    #[test]
    fn test_search_models_rejects_invalid_ordering() {
        let client = PrintablesClient::new().unwrap();
        let err = client.search_models("benchy", 5, "invalid_order").unwrap_err();
        assert!(err.contains("Invalid ordering"));
    }

    #[test]
    fn test_live_graphql_metadata_files_and_images() {
        let client = PrintablesClient::new().unwrap();
        let meta = client.get_model_metadata("1648230").expect("metadata fetch failed");
        assert_eq!(meta.id, "1648230");
        assert!(meta.name.to_lowercase().contains("benchy"));
        assert!(!meta.tags.is_empty());
        assert!(meta.main_image_url.is_some());
        assert!(!meta.description.is_empty());

        let files = client.get_model_files("1648230").expect("files fetch failed");
        assert!(!files.is_empty());
        assert!(files.iter().any(|f| f.download_url.is_some()));

        let images = client.get_model_images("1648230").expect("images fetch failed");
        assert!(!images.is_empty());
        assert!(images[0].url.starts_with("https://media.printables.com/"));
    }
}

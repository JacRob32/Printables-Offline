#!/usr/bin/env python3
"""
Printables Offline — Clone Adapter
===================================
Thin adapter over the vendored printables_api.py that adds:
  • URL-driven clone entrypoint (parse model_id + slug from printables.com/model/…)
  • File/image downloader with per-file progress reporting
  • metadata.json writer matching the Stage-3 contract
  • NDJSON stdout stream for Rust-side progress parsing

Usage (invoked by Rust):
    python3 printables_clone.py clone \
        --url <https://www.printables.com/model/ID-slug> \
        --dest <absolute/path/to/library/folder> \
        [--python <path-to-interpreter>] \
        [--debug]

NDJSON record shapes on stdout:
    {"kind":"phase","phase":"metadata|files|images|finalize","percent":N,"message":"…"}
    {"kind":"file_progress","file":"x.3mf","index":2,"total":5,"bytes_done":N,"bytes_total":N}
    {"kind":"done","model_dir":"/abs/path","metadata_path":"/abs/path/metadata.json"}
    {"kind":"error","code":"NETWORK|PARSE|DISK|HTTP","message":"…"}
"""

import argparse
import json
import os
import re
import sys
import time
from pathlib import Path
from urllib.parse import urlparse

# ── Vendored API import ──────────────────────────────────────────────
# printables_api.py lives at the repo root, one level up from this script.
# We add the parent directory to path so `import printables_api` works.
_script_dir = os.path.dirname(os.path.abspath(__file__))
_repo_root = os.path.dirname(_script_dir)
sys.path.insert(0, _repo_root)
import printables_api as api  # noqa: E402


# ── Helpers ──────────────────────────────────────────────────────────
MODEL_URL_RE = re.compile(
    r"https?://(?:www\.)?printables\.com/model/(?P<id>\d+)(?:-(?P<slug>[^/?]+))?"
)


def parse_model_url(url: str) -> dict | None:
    """Return {id, slug} or None."""
    m = MODEL_URL_RE.match(url.strip())
    if not m:
        return None
    return {"id": m.group("id"), "slug": m.group("slug") or ""}


def emit(record: dict) -> None:
    """Write one NDJSON line to stdout (flushed immediately)."""
    sys.stdout.write(json.dumps(record, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def safe_filename(name: str) -> str:
    """Strip characters unsafe for all three OS filesystems."""
    return re.sub(r'[<>:"/\\|?*]', "_", name)[:120]


def download_file(url: str, dest: Path, debug: bool = False) -> int | None:
    """Download a single file, returning bytes written or None on failure."""
    try:
        resp = api.requests.get(url, stream=True, timeout=60)
        resp.raise_for_status()
        total = int(resp.headers.get("content-length", 0))
        done = 0
        with open(dest, "wb") as f:
            for chunk in resp.iter_content(chunk_size=8192):
                if chunk:
                    f.write(chunk)
                    done += len(chunk)
                    if total > 0 and debug:
                        pct = int(done / total * 100)
                        emit({
                            "kind": "file_progress",
                            "file": dest.name,
                            "bytes_done": done,
                            "bytes_total": total,
                            "percent": pct,
                        })
        return done
    except Exception as e:
        if debug:
            emit({"kind": "error", "code": "DOWNLOAD", "message": str(e)})
        return None


# ── Main clone pipeline ──────────────────────────────────────────────
def run_clone(url: str, dest_root: str, debug: bool = False) -> None:
    parsed = parse_model_url(url)
    if not parsed:
        emit({"kind": "error", "code": "PARSE", "message": f"Not a valid printables.com model URL: {url}"})
        sys.exit(1)

    model_id = parsed["id"]
    slug = parsed["slug"]
    model_dir = Path(dest_root) / f"{model_id}-{slug}"
    files_dir = model_dir / "files"
    images_dir = model_dir / "images"

    try:
        files_dir.mkdir(parents=True, exist_ok=True)
        images_dir.mkdir(parents=True, exist_ok=True)
    except OSError as e:
        emit({"kind": "error", "code": "DISK", "message": f"Cannot create directories: {e}"})
        sys.exit(1)

    # Phase 1 – Metadata
    emit({"kind": "phase", "phase": "metadata", "percent": 5, "message": "Fetching model metadata…"})
    model_url = f"https://www.printables.com/model/{model_id}-{slug}"

    description = api.get_model_description(model_url, debug=debug)

    # Extend search fragment to include tags via a direct GraphQL call
    tags_query = """
    query ModelTags($id: ID!) {
      model: print(id: $id) {
        id name slug ratingAvg likesCount downloadCount datePublished
        user { publicUsername __typename }
        image { filePath }
        tags { name __typename }
        __typename
      }
    }
    """
    headers = {"User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"}
    payload = {"operationName": "ModelTags", "query": tags_query, "variables": {"id": model_id}}
    tags_resp = api.requests.post("https://api.printables.com/graphql/", headers=headers, json=payload, timeout=15)
    tags_data = tags_resp.json().get("data", {}).get("model", {}) if tags_resp.status_code == 200 else {}
    tag_names = [t["name"] for t in tags_data.get("tags", [])]

    main_image_url = None
    img_info = tags_data.get("image", {})
    if img_info and img_info.get("filePath"):
        main_image_url = "https://media.printables.com/" + img_info["filePath"]

    emit({"kind": "phase", "phase": "metadata", "percent": 20, "message": "Metadata fetched"})

    # Phase 2 – Files
    emit({"kind": "phase", "phase": "files", "percent": 25, "message": "Resolving download links…"})
    files_with_links = api.get_model_files(model_id, debug=debug)
    if not files_with_links:
        emit({"kind": "phase", "phase": "files", "percent": 30, "message": "No downloadable files found"})

    downloaded_files = []
    for idx, f in enumerate(files_with_links):
        fname = safe_filename(f["name"])
        dest = files_dir / fname
        emit({
            "kind": "phase", "phase": "files",
            "percent": 30 + int((idx + 1) / max(len(files_with_links), 1) * 40),
            "message": f"Downloading {fname}…",
            "file": fname, "file_index": idx + 1, "file_total": len(files_with_links),
        })
        size = download_file(f["download_url"], dest, debug=debug) if f.get("download_url") else None
        if size is not None:
            downloaded_files.append({
                "name": fname,
                "kind": f["file_type"],
                "size_bytes": size,
                "local_path": f"files/{fname}",
            })
        elif debug:
            emit({"kind": "error", "code": "DOWNLOAD", "message": f"Failed: {fname}"})

    emit({"kind": "phase", "phase": "files", "percent": 70, "message": f"{len(downloaded_files)} files downloaded"})

    # Phase 3 – Images
    emit({"kind": "phase", "phase": "images", "percent": 75, "message": "Downloading cover image…"})
    cover_path = None
    if main_image_url:
        ext = main_image_url.split(".")[-1].split("?")[0][:4] or "jpg"
        cover_name = f"cover.{ext}"
        cover_dest = images_dir / cover_name
        sz = download_file(main_image_url, cover_dest, debug=debug)
        if sz is not None:
            cover_path = f"images/{cover_name}"
    emit({"kind": "phase", "phase": "images", "percent": 85, "message": "Images complete"})

    # Phase 4 – Finalize metadata.json
    emit({"kind": "phase", "phase": "finalize", "percent": 90, "message": "Writing metadata.json…"})
    metadata = {
        "schema_version": 1,
        "source": "printables.com",
        "model_id": model_id,
        "slug": slug,
        "name": tags_data.get("name", url),
        "url": model_url,
        "author": tags_data.get("user", {}).get("publicUsername", "unknown"),
        "tags": tag_names,
        "stats": {
            "likes": tags_data.get("likesCount"),
            "downloads": tags_data.get("downloadCount"),
            "rating": tags_data.get("ratingAvg"),
            "published_date": tags_data.get("datePublished"),
        },
        "description": description,
        "cloned_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "files": downloaded_files,
        "images": [{"name": cover_path.split("/")[-1], "kind": "cover", "local_path": cover_path}] if cover_path else [],
    }

    meta_path = model_dir / "metadata.json"
    with open(meta_path, "w", encoding="utf-8") as fh:
        json.dump(metadata, fh, ensure_ascii=False, indent=2)

    emit({
        "kind": "done",
        "model_dir": str(model_dir.resolve()),
        "metadata_path": str(meta_path.resolve()),
        "percent": 100,
    })


# ── CLI entrypoint ───────────────────────────────────────────────────
if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Clone a Printables.com model into a local library folder.")
    sub = parser.add_subparsers(dest="command")

    clone_p = sub.add_parser("clone", help="Clone a single model by URL")
    clone_p.add_argument("--url", required=True, help="Full printables.com model URL")
    clone_p.add_argument("--dest", required=True, help="Absolute path to the parent library folder")
    clone_p.add_argument("--debug", action="store_true", help="Emit verbose NDJSON progress")

    args = parser.parse_args()

    if args.command == "clone":
        run_clone(args.url, args.dest, debug=args.debug)
    else:
        parser.print_help()
        sys.exit(1)

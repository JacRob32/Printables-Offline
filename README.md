# Printables Offline

A desktop app for downloading and managing 3D printing models from Printables.com. Clone models to your local library, browse them offline, and open files directly in your slicer.

<!-- Replace this with a screenshot of the app -->
<!-- [screenshot.png](screenshot.png) -->

## Features

- **Clone models** — Paste a Printables.com URL and the app downloads the model files, cover image, tags, and metadata to your local library.
- **Local library** — Browse all cloned models in a grid view with cover images, file counts, and sizes. Search and filter by file type.
- **Detail view** — View model description, tags, file list, and source URL for each model.
- **Open in slicer** — Launch your configured slicer (PrusaSlicer, OrcaSlicer, Bambu Studio, Cura) with model files directly from the app.
- **Export** — Copy model files to any destination folder.
- **Delete** — Remove models from your library.
- **Settings** — Configure library folder, slicer executable, and theme (light/dark/system). Preferences persist across sessions.

## How It Works

The app uses a Python scraper (bundled in `py/`) that calls the Printables GraphQL API to fetch model metadata, generate download links, and retrieve cover images. The Rust/Tauri backend manages the UI, file system operations, and launches external processes.

All data is stored locally. No account or API key is required.

## Requirements

- macOS (Apple Silicon or Intel)
- Python 3.10+ with `cloudscraper`, `beautifulsoup4`, and `requests` installed
- A slicer application (optional, for the "Open in Slicer" feature)

## Install

Download the latest `.dmg` from [Releases](#releases), open it, and drag **Printables Offline.app** to your Applications folder.

On first launch, go to **Preferences** and set your library folder. This is where cloned models will be stored.

## Build from Source

```bash
# Clone the repo
git clone https://github.com/JacRob32/Printables-Offline.git
cd Printables-Offline

# Install Python dependencies
pip3 install cloudscraper beautifulsoup4 requests

# Build the app
cd src-tauri
cargo tauri build
```

The `.app` and `.dmg` will be in `src-tauri/target/release/bundle/`.

For development with hot reload:

```bash
cargo tauri dev
```

## Settings Storage

Preferences are saved to `~/.printablesoffline/prefs.json`. You can edit this file directly if needed.

## Platform Support

Currently built for macOS only. A Windows version is planned.

## Releases

| Version | Date | Notes |
|---------|------|-------|
| 0.1.0 | 2026-08-24 | Initial release. Model cloning, local library, slicer integration, settings persistence. |

Download: [Printables Offline_0.1.0_aarch64.dmg](https://github.com/JacRob32/Printables-Offline/releases)

## License

MIT

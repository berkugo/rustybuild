# RustyBuild — GUI

Desktop UI built with Tauri 2 + React + Tailwind. View/edit build.toml, dependency graph, and run builds.

## Features

- **Load build.toml** — By path or file picker
- **Dependency graph** — Targets and `deps` relationships (ReactFlow)
- **build.toml editor** — Edit and save content
- **Build** — "Build" / "Clean + Build" to see output in the same window

## Requirements

- Node.js 18+
- Rust + Cargo
- **Linux:** Tauri 2 needs WebKit2GTK **4.1** (libsoup3). Install per distro:
  ```bash
  # Fedora (webkit2gtk4.1 available)
  sudo dnf install -y glib2-devel libsoup3-devel javascriptcoregtk4.1-devel webkit2gtk4.1-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel patchelf

  # Debian/Ubuntu
  sudo apt install -y libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev librsvg2-dev patchelf
  ```
  **AlmaLinux 9 / RHEL 9:** Default repos only have webkit2gtk**3**; webkit2gtk**4.1** is not available, so `javascriptcore-rs-sys` may fail to find the library. Options:
  - **Use another distro in WSL:** e.g. Ubuntu or Fedora WSL and install the packages above.
  - **Use CLI only:** Run the tool from the project root with `cargo run -- -c build.toml`; no GUI.
  - **Run frontend in browser:** `cd gui && npm run dev` → http://localhost:5173 (UI loads but "Load"/"Build" need Tauri to work).

## Install and run

```bash
cd gui
npm install
npm run tauri dev
```

To run only the frontend (no Tauri window):

```bash
npm run dev
```

Opens http://localhost:5173 in the browser; Tauri commands (`invoke`) only work inside Tauri, so "Load" / "Build" work only with `npm run tauri dev`.

## Project structure

- `src/` — React (Vite) app
  - `App.tsx` — Main page, tabs, Tauri commands
  - `components/DependencyGraph.tsx` — DAG with ReactFlow
  - `components/BuildTomlEditor.tsx` — build.toml text area
  - `components/BuildLog.tsx` — Build output
- `src-tauri/` — Tauri (Rust) backend
  - `src/lib.rs` — Commands: `parse_build_toml`, `read_file`, `write_file`, `run_build`
  - Uses the `cpp_build_tool` library (parent directory)

## Usage

1. **Load** — Enter e.g. `example_complex/build.toml` or an absolute path in "build.toml or full path", then click "Load".
2. **Dependency graph** — Nodes are targets, edges are deps; colors by type (green: executable, blue: static_lib, purple: shared_lib).
3. **build.toml** — Edit content and save to disk; the page re-parses.
4. **Build** — Use "Build" or "Clean + Build" and view output in the "Build output" tab.

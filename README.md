# tview

[![CI](https://github.com/mashu/tview/actions/workflows/ci.yml/badge.svg)](https://github.com/mashu/tview/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/mashu/tview/branch/main/graph/badge.svg)](https://codecov.io/gh/mashu/tview)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://github.com/mashu/tview/actions/workflows/release.yml/badge.svg)](https://github.com/mashu/tview/actions/workflows/release.yml)

Fast viewer for plotting and comparing CSV/TSV metric logs — native desktop UI or headless web server, sharing one threaded backend.

## Install

Download a binary for your OS from [Releases](https://github.com/mashu/tview/releases), or build from source:

```bash
cargo install --path .
```

Tagged releases (`v*`) build Linux, Windows, and macOS ARM binaries and publish them automatically.

## Usage

### Desktop

```bash
tview
tview run_a.csv run_b.tsv
```

Loading, live file watching, and PNG export run on a background worker so the UI stays responsive.

### Web (headless)

No display required — useful on remote machines:

```bash
tview serve --bind 0.0.0.0:8080
tview serve --bind 127.0.0.1:8080 ./metrics.csv
```

Open `http://<host>:8080`. Charts render in the browser with [Apache ECharts](https://echarts.apache.org/) (canvas); the server only sends JSON series data. PNG export is on demand.

## Architecture

- **backend** — owns data; load / watch / export on a worker thread; publishes snapshots
- **desktop** — egui frontend (commands in, snapshot out)
- **web** — axum HTTP API + embedded UI (same backend)

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release -- fixtures/example_run_a.csv
cargo run --release -- serve --bind 127.0.0.1:8080 fixtures/example_run_a.csv
```

## License

MIT

# tview

[![CI](https://github.com/mashu/tview/actions/workflows/ci.yml/badge.svg)](https://github.com/mashu/tview/actions/workflows/ci.yml)
[![codecov](https://codecov.io/gh/mashu/tview/branch/main/graph/badge.svg)](https://codecov.io/gh/mashu/tview)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Release](https://github.com/mashu/tview/actions/workflows/release.yml/badge.svg)](https://github.com/mashu/tview/actions/workflows/release.yml)

Fast desktop viewer for plotting and comparing CSV/TSV metric logs.

Open one or more training or experiment logs, pick columns, and overlay series with optional smoothing, log-scale Y, and PNG export.

## Install

Download a binary for your OS from [Releases](https://github.com/mashu/tview/releases), or build from source:

```bash
cargo install --path .
```

## Usage

```bash
tview
tview run_a.csv run_b.tsv
```

- **Add files** from the toolbar, or drag & drop CSV/TSV onto the window
- Choose an **X axis** (step/epoch/row index) and tick columns to plot
- Adjust **smoothing** / **line width**, toggle **log Y**, then **Export PNG**
- Keep **Live reload** on to watch open files and re-plot as new rows are appended

## Develop

```bash
cargo test
cargo clippy --all-targets -- -D warnings
cargo run --release -- fixtures/example_run_a.csv
```

## License

MIT

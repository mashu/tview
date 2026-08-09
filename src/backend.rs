//! Threaded application backend: owns data, performs I/O, publishes UI snapshots.
//!
//! Desktop and web frontends send [`Command`]s and read [`SharedView`] — they never
//! touch the filesystem or block on CSV parsing / PNG export.

use crate::data::DataFile;
use crate::export::{export_png, render_png_bytes};
use crate::series::{ROW_INDEX, Series, build_series, x_axis_options};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender, TrySendError};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const LIVE_POLL_INTERVAL: Duration = Duration::from_millis(500);
const EVENT_CHANNEL_CAP: usize = 64;

/// Plot / display options owned by the backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlotOptions {
    pub x_col: String,
    pub log_y: bool,
    pub smoothing: f64,
    pub line_w: f32,
}

impl Default for PlotOptions {
    fn default() -> Self {
        Self {
            x_col: ROW_INDEX.to_string(),
            log_y: false,
            smoothing: 0.0,
            line_w: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ColumnView {
    pub name: String,
    pub numeric: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileView {
    pub name: String,
    pub path: String,
    pub nrows: usize,
    pub visible: bool,
    pub columns: Vec<ColumnView>,
}

/// Read-only snapshot consumed by desktop / web UIs.
#[derive(Debug, Clone, Serialize)]
pub struct SharedView {
    pub generation: u64,
    pub status: String,
    pub busy: bool,
    pub live_reload: bool,
    pub options: PlotOptions,
    pub x_axis_options: Vec<String>,
    pub files: Vec<FileView>,
    pub series: Vec<SeriesDto>,
}

impl Default for SharedView {
    fn default() -> Self {
        Self {
            generation: 0,
            status: String::new(),
            busy: false,
            live_reload: true,
            options: PlotOptions::default(),
            x_axis_options: vec![ROW_INDEX.to_string()],
            files: Vec::new(),
            series: Vec::new(),
        }
    }
}

/// JSON-friendly series (also used to rebuild egui plot points).
#[derive(Debug, Clone, Serialize)]
pub struct SeriesDto {
    pub name: String,
    pub color: (u8, u8, u8),
    pub xs: Vec<f64>,
    pub ys: Vec<f64>,
    pub raw_ys: Option<Vec<f64>>,
}

impl From<&Series> for SeriesDto {
    fn from(s: &Series) -> Self {
        Self {
            name: s.name.clone(),
            color: s.color,
            xs: s.xs.clone(),
            ys: s.ys.clone(),
            raw_ys: s.raw_ys.clone(),
        }
    }
}

impl SeriesDto {
    pub fn to_series(&self) -> Series {
        Series {
            name: self.name.clone(),
            color: self.color,
            xs: self.xs.clone(),
            ys: self.ys.clone(),
            raw_ys: self.raw_ys.clone(),
        }
    }
}

/// Commands sent from any frontend to the worker thread.
#[derive(Debug)]
pub enum Command {
    LoadPaths(Vec<PathBuf>),
    /// Load content that was uploaded / provided as bytes (web).
    LoadBytes {
        name: String,
        bytes: Vec<u8>,
    },
    RemoveFile(usize),
    SetVisible {
        index: usize,
        visible: bool,
    },
    SetColumnSelected {
        file: usize,
        column: usize,
        selected: bool,
    },
    SetOptions(PlotOptions),
    SetLiveReload(bool),
    /// Native/desktop export to a filesystem path.
    ExportPng {
        path: PathBuf,
        size: (u32, u32),
    },
    /// Render PNG and deliver bytes via [`Event::PngReady`].
    RenderPng {
        request_id: u64,
        size: (u32, u32),
    },
    Shutdown,
}

/// Events emitted by the worker (status, export results).
#[derive(Debug, Clone)]
pub enum Event {
    Status(String),
    ExportSaved(PathBuf),
    PngReady { request_id: u64, png: Arc<Vec<u8>> },
    Failed(String),
}

/// Handle used by frontends to talk to the backend worker.
#[derive(Clone)]
pub struct Backend {
    cmd_tx: Sender<Command>,
    event_rx: Arc<std::sync::Mutex<Receiver<Event>>>,
    view: Arc<RwLock<SharedView>>,
    running: Arc<AtomicBool>,
}

impl Backend {
    pub fn start() -> (Self, JoinHandle<()>) {
        let (cmd_tx, cmd_rx) = mpsc::channel::<Command>();
        let (event_tx, event_rx) = mpsc::sync_channel::<Event>(EVENT_CHANNEL_CAP);
        let view = Arc::new(RwLock::new(SharedView::default()));
        let running = Arc::new(AtomicBool::new(true));

        let view_worker = Arc::clone(&view);
        let running_worker = Arc::clone(&running);
        let handle = thread::Builder::new()
            .name("tview-backend".into())
            .spawn(move || {
                worker_loop(cmd_rx, event_tx, view_worker, running_worker);
            })
            .expect("spawn backend worker");

        let backend = Self {
            cmd_tx,
            event_rx: Arc::new(std::sync::Mutex::new(event_rx)),
            view,
            running,
        };
        (backend, handle)
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn view(&self) -> Arc<RwLock<SharedView>> {
        Arc::clone(&self.view)
    }

    pub fn snapshot(&self) -> SharedView {
        self.view.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Lightweight poll fields — avoids cloning series payloads.
    pub fn meta(&self) -> (u64, bool, String, usize, usize) {
        match self.view.read() {
            Ok(g) => (
                g.generation,
                g.busy,
                g.status.clone(),
                g.files.len(),
                g.series.len(),
            ),
            Err(_) => (0, false, String::new(), 0, 0),
        }
    }

    /// Drain pending worker events (non-blocking).
    pub fn poll_events(&self) -> Vec<Event> {
        let Ok(rx) = self.event_rx.lock() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            out.push(ev);
        }
        out
    }

    pub fn shutdown(self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(Command::Shutdown);
    }
}

struct WorkerState {
    files: Vec<DataFile>,
    options: PlotOptions,
    live_reload: bool,
    busy: bool,
    status: String,
    series: Vec<Series>,
    generation: u64,
}

impl WorkerState {
    fn new() -> Self {
        Self {
            files: Vec::new(),
            options: PlotOptions::default(),
            live_reload: true,
            busy: false,
            status: String::new(),
            series: Vec::new(),
            generation: 0,
        }
    }

    fn rebuild_series(&mut self) {
        self.series = build_series(
            &self.files,
            &self.options.x_col,
            self.options.log_y,
            self.options.smoothing,
        );
    }

    fn publish(&mut self, view: &RwLock<SharedView>) {
        self.generation = self.generation.wrapping_add(1);
        let files: Vec<FileView> = self
            .files
            .iter()
            .map(|f| FileView {
                name: f.name.clone(),
                path: f.path.display().to_string(),
                nrows: f.nrows,
                visible: f.visible,
                columns: f
                    .columns
                    .iter()
                    .zip(f.selected.iter())
                    .map(|(c, &selected)| ColumnView {
                        name: c.name.clone(),
                        numeric: c.numeric,
                        selected,
                    })
                    .collect(),
            })
            .collect();
        let snapshot = SharedView {
            generation: self.generation,
            status: self.status.clone(),
            busy: self.busy,
            live_reload: self.live_reload,
            options: self.options.clone(),
            x_axis_options: x_axis_options(&self.files),
            files,
            series: self.series.iter().map(SeriesDto::from).collect(),
        };
        if let Ok(mut g) = view.write() {
            *g = snapshot;
        }
    }

    fn set_status(
        &mut self,
        msg: impl Into<String>,
        view: &RwLock<SharedView>,
        events: &SyncSender<Event>,
    ) {
        self.status = msg.into();
        let _ = emit(events, Event::Status(self.status.clone()));
        self.publish(view);
    }

    fn after_files_changed(&mut self) {
        let opts = x_axis_options(&self.files);
        if !opts.iter().any(|o| o == &self.options.x_col) {
            self.options.x_col = ROW_INDEX.to_string();
        }
        if self.options.x_col == ROW_INDEX
            && let Some(x) = opts
                .iter()
                .find(|o| matches!(o.as_str(), "step" | "epoch" | "iter" | "iteration"))
        {
            self.options.x_col = x.clone();
        }
        self.rebuild_series();
    }

    fn default_select_last(&mut self) {
        let Some(f) = self.files.last_mut() else {
            return;
        };
        if f.selected.iter().any(|&b| b) {
            return;
        }
        let pick = f
            .columns
            .iter()
            .position(|c| c.numeric && c.name.eq_ignore_ascii_case("loss"))
            .or_else(|| {
                f.columns.iter().position(|c| {
                    c.numeric && !matches!(c.name.as_str(), "step" | "epoch" | "iter" | "iteration")
                })
            });
        if let Some(i) = pick {
            f.selected[i] = true;
        }
    }
}

fn emit(tx: &SyncSender<Event>, ev: Event) -> Result<(), ()> {
    match tx.try_send(ev) {
        Ok(()) => Ok(()),
        Err(TrySendError::Full(ev)) => {
            // Drop oldest-style: try once more after a brief wait is overkill; just drop.
            let _ = ev;
            Ok(())
        }
        Err(TrySendError::Disconnected(_)) => Err(()),
    }
}

fn worker_loop(
    cmd_rx: Receiver<Command>,
    event_tx: SyncSender<Event>,
    view: Arc<RwLock<SharedView>>,
    running: Arc<AtomicBool>,
) {
    let mut state = WorkerState::new();
    state.publish(&view);

    while running.load(Ordering::SeqCst) {
        match cmd_rx.recv_timeout(LIVE_POLL_INTERVAL) {
            Ok(Command::Shutdown) => break,
            Ok(cmd) => {
                handle_command(cmd, &mut state, &view, &event_tx);
            }
            Err(RecvTimeoutError::Timeout) => {
                if state.live_reload {
                    poll_live(&mut state, &view, &event_tx);
                }
            }
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    running.store(false, Ordering::SeqCst);
}

fn poll_live(state: &mut WorkerState, view: &RwLock<SharedView>, events: &SyncSender<Event>) {
    let mut updated = 0usize;
    let mut rows = 0usize;
    for f in &mut state.files {
        match f.refresh_from_disk() {
            Ok(crate::data::RefreshOutcome::Reloaded) => {
                updated += 1;
                rows += f.nrows;
            }
            Ok(crate::data::RefreshOutcome::Unchanged) => {}
            Err(e) => {
                state.set_status(format!("Watch error: {e}"), view, events);
                return;
            }
        }
    }
    if updated > 0 {
        state.rebuild_series();
        let msg = if updated == 1 {
            format!("Live update · {rows} rows")
        } else {
            format!("Live update · {updated} files refreshed")
        };
        state.set_status(msg, view, events);
    }
}

fn handle_command(
    cmd: Command,
    state: &mut WorkerState,
    view: &RwLock<SharedView>,
    events: &SyncSender<Event>,
) {
    match cmd {
        Command::Shutdown => {}
        Command::LoadPaths(paths) => {
            state.busy = true;
            state.publish(view);
            let mut loaded = 0usize;
            let mut last_name = String::new();
            for p in paths {
                match DataFile::load(&p) {
                    Ok(f) => {
                        last_name = f.name.clone();
                        state.files.push(f);
                        state.default_select_last();
                        loaded += 1;
                    }
                    Err(e) => {
                        state.busy = false;
                        state.set_status(
                            format!("Error loading {}: {e}", p.display()),
                            view,
                            events,
                        );
                        let _ = emit(events, Event::Failed(e));
                        return;
                    }
                }
            }
            state.after_files_changed();
            state.busy = false;
            let msg = if loaded > 1 {
                format!("Loaded {loaded} files")
            } else if loaded == 1 {
                format!("Loaded {last_name}")
            } else {
                "No files loaded".into()
            };
            state.set_status(msg, view, events);
        }
        Command::LoadBytes { name, bytes } => {
            state.busy = true;
            state.publish(view);
            let text = match String::from_utf8(bytes) {
                Ok(t) => t,
                Err(e) => {
                    state.busy = false;
                    state.set_status(format!("Invalid UTF-8 in {name}: {e}"), view, events);
                    return;
                }
            };
            let path = PathBuf::from(&name);
            match DataFile::parse(&path, &text) {
                Ok(mut f) => {
                    // Uploaded files have no stable disk watch target.
                    f.file_len = 0;
                    f.mtime = None;
                    state.files.push(f);
                    state.default_select_last();
                    state.after_files_changed();
                    state.busy = false;
                    state.set_status(format!("Loaded {name}"), view, events);
                }
                Err(e) => {
                    state.busy = false;
                    state.set_status(format!("Error loading {name}: {e}"), view, events);
                    let _ = emit(events, Event::Failed(e));
                }
            }
        }
        Command::RemoveFile(i) => {
            if i < state.files.len() {
                let name = state.files[i].name.clone();
                state.files.remove(i);
                state.after_files_changed();
                state.set_status(format!("Removed {name}"), view, events);
            }
        }
        Command::SetVisible { index, visible } => {
            if let Some(f) = state.files.get_mut(index) {
                f.visible = visible;
                state.rebuild_series();
                state.publish(view);
            }
        }
        Command::SetColumnSelected {
            file,
            column,
            selected,
        } => {
            if let Some(f) = state.files.get_mut(file)
                && let Some(slot) = f.selected.get_mut(column)
            {
                *slot = selected;
                state.rebuild_series();
                state.publish(view);
            }
        }
        Command::SetOptions(opts) => {
            state.options = opts;
            state.rebuild_series();
            state.publish(view);
        }
        Command::SetLiveReload(v) => {
            state.live_reload = v;
            state.publish(view);
        }
        Command::ExportPng { path, size } => {
            state.busy = true;
            state.publish(view);
            let lw = state.options.line_w.round() as u32;
            let result = export_png(
                &path,
                &state.series,
                &state.options.x_col,
                state.options.log_y,
                size,
                lw.max(1),
            );
            state.busy = false;
            match result {
                Ok(()) => {
                    state.set_status(format!("Saved {}", path.display()), view, events);
                    let _ = emit(events, Event::ExportSaved(path));
                }
                Err(e) => {
                    state.set_status(format!("Export failed: {e}"), view, events);
                    let _ = emit(events, Event::Failed(e));
                }
            }
        }
        Command::RenderPng { request_id, size } => {
            state.busy = true;
            state.publish(view);
            let lw = state.options.line_w.round() as u32;
            let result = render_png_bytes(
                &state.series,
                &state.options.x_col,
                state.options.log_y,
                size,
                lw.max(1),
            );
            state.busy = false;
            state.publish(view);
            match result {
                Ok(png) => {
                    let _ = emit(
                        events,
                        Event::PngReady {
                            request_id,
                            png: Arc::new(png),
                        },
                    );
                }
                Err(e) => {
                    state.set_status(format!("Render failed: {e}"), view, events);
                    let _ = emit(events, Event::Failed(e));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::time::Duration;

    #[test]
    fn backend_loads_file_off_caller_thread() {
        let dir = std::env::temp_dir().join(format!("tview-backend-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("run.csv");
        {
            let mut f = std::fs::File::create(&path).unwrap();
            writeln!(f, "step,loss").unwrap();
            writeln!(f, "1,1.0").unwrap();
            writeln!(f, "2,0.5").unwrap();
        }

        let (backend, handle) = Backend::start();
        backend.send(Command::LoadPaths(vec![path.clone()]));

        let mut ok = false;
        for _ in 0..50 {
            let snap = backend.snapshot();
            if !snap.files.is_empty() && !snap.busy {
                assert_eq!(snap.files[0].nrows, 2);
                assert!(!snap.series.is_empty());
                ok = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(ok, "backend did not load file in time");

        backend.shutdown();
        let _ = handle.join();
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }
}

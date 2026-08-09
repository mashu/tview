use clap::{Parser, Subcommand};
use std::net::SocketAddr;
use std::path::PathBuf;
use tview::backend::{Backend, Command};
use tview::desktop::{self, DesktopApp};

#[derive(Parser, Debug)]
#[command(
    name = "tview",
    about = "Plot and compare CSV/TSV metric logs (desktop UI or headless web server)"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// CSV/TSV files to open (desktop mode when no subcommand is given)
    #[arg(global = true)]
    files: Vec<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the native desktop UI
    Desktop {
        /// CSV/TSV files to open
        files: Vec<PathBuf>,
    },
    /// Run a headless HTTP UI (no display required)
    Serve {
        /// Bind address, e.g. 127.0.0.1:8080 or 0.0.0.0:8080
        #[arg(long, default_value = "127.0.0.1:8080")]
        bind: String,
        /// CSV/TSV files to preload from the server filesystem
        files: Vec<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();
    match cli.command {
        None => run_desktop(cli.files),
        Some(Commands::Desktop { files }) => run_desktop(files),
        Some(Commands::Serve { bind, files }) => {
            let addr: SocketAddr = bind.parse().unwrap_or_else(|e| {
                eprintln!("invalid --bind {bind}: {e}");
                std::process::exit(2);
            });
            run_serve(addr, files);
        }
    }
}

fn existing_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().filter(|p| p.is_file()).collect()
}

fn run_desktop(files: Vec<PathBuf>) {
    let initial = existing_files(files);
    let (backend, _worker) = Backend::start();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1360.0, 860.0])
            .with_min_inner_size([800.0, 520.0])
            .with_title("tview"),
        ..Default::default()
    };
    let result = eframe::run_native(
        "tview",
        options,
        Box::new(move |cc| {
            desktop::theme::apply(&cc.egui_ctx);
            Box::new(DesktopApp::new(backend, initial))
        }),
    );
    if let Err(e) = result {
        eprintln!("tview desktop error: {e}");
        std::process::exit(1);
    }
}

fn run_serve(bind: SocketAddr, files: Vec<PathBuf>) {
    let initial = existing_files(files);
    let (backend, worker) = Backend::start();
    if !initial.is_empty() {
        backend.send(Command::LoadPaths(initial));
    }
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    let result = rt.block_on(tview::web::serve(backend.clone(), bind));
    backend.shutdown();
    let _ = worker.join();
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

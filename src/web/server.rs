use crate::backend::{Backend, Command, PlotOptions};
use crate::export::render_png_bytes;
use axum::extract::{Multipart, Path, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::CorsLayer;

#[derive(Clone)]
struct AppState {
    backend: Backend,
}

pub async fn serve(backend: Backend, bind: SocketAddr) -> Result<(), String> {
    let state = AppState { backend };
    let app = Router::new()
        .route("/", get(index))
        .route("/api/state", get(get_state))
        .route("/api/options", post(set_options))
        .route("/api/live", post(set_live))
        .route("/api/load", post(load_paths))
        .route("/api/upload", post(upload))
        .route("/api/files/{index}", delete(remove_file))
        .route("/api/files/{index}/visible", post(set_visible))
        .route("/api/files/{file}/columns/{column}", post(set_column))
        .route("/api/plot.png", get(plot_png))
        .layer(CorsLayer::permissive())
        .with_state(Arc::new(state));

    let listener = tokio::net::TcpListener::bind(bind)
        .await
        .map_err(|e| format!("bind {bind}: {e}"))?;
    eprintln!("tview web UI listening on http://{bind}");
    axum::serve(listener, app)
        .await
        .map_err(|e| format!("server error: {e}"))
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn get_state(State(st): State<Arc<AppState>>) -> impl IntoResponse {
    Json(st.backend.snapshot())
}

async fn set_options(State(st): State<Arc<AppState>>, Json(opts): Json<PlotOptions>) -> StatusCode {
    st.backend.send(Command::SetOptions(opts));
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct LiveBody {
    live: bool,
}

async fn set_live(State(st): State<Arc<AppState>>, Json(body): Json<LiveBody>) -> StatusCode {
    st.backend.send(Command::SetLiveReload(body.live));
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct LoadBody {
    paths: Vec<String>,
}

async fn load_paths(State(st): State<Arc<AppState>>, Json(body): Json<LoadBody>) -> StatusCode {
    let paths: Vec<PathBuf> = body.paths.into_iter().map(PathBuf::from).collect();
    st.backend.send(Command::LoadPaths(paths));
    StatusCode::ACCEPTED
}

async fn upload(
    State(st): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<StatusCode, (StatusCode, String)> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let name = field
            .file_name()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "upload.csv".into());
        let bytes = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
            .to_vec();
        st.backend.send(Command::LoadBytes { name, bytes });
    }
    Ok(StatusCode::ACCEPTED)
}

async fn remove_file(State(st): State<Arc<AppState>>, Path(index): Path<usize>) -> StatusCode {
    st.backend.send(Command::RemoveFile(index));
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct VisibleBody {
    visible: bool,
}

async fn set_visible(
    State(st): State<Arc<AppState>>,
    Path(index): Path<usize>,
    Json(body): Json<VisibleBody>,
) -> StatusCode {
    st.backend.send(Command::SetVisible {
        index,
        visible: body.visible,
    });
    StatusCode::NO_CONTENT
}

#[derive(Deserialize)]
struct SelectedBody {
    selected: bool,
}

async fn set_column(
    State(st): State<Arc<AppState>>,
    Path((file, column)): Path<(usize, usize)>,
    Json(body): Json<SelectedBody>,
) -> StatusCode {
    st.backend.send(Command::SetColumnSelected {
        file,
        column,
        selected: body.selected,
    });
    StatusCode::NO_CONTENT
}

async fn plot_png(State(st): State<Arc<AppState>>) -> Response {
    let snap = st.backend.snapshot();
    if snap.series.is_empty() {
        return (StatusCode::NOT_FOUND, "no series selected").into_response();
    }
    let series: Vec<_> = snap.series.iter().map(|s| s.to_series()).collect();
    let x_col = snap.options.x_col.clone();
    let log_y = snap.options.log_y;
    let lw = snap.options.line_w.round() as u32;
    let rendered = tokio::task::spawn_blocking(move || {
        render_png_bytes(&series, &x_col, log_y, (1280, 800), lw.max(1))
    })
    .await;

    match rendered {
        Ok(Ok(png)) => (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, "image/png"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            png,
        )
            .into_response(),
        Ok(Err(e)) => (StatusCode::BAD_REQUEST, e).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

use std::{collections::HashMap, sync::Arc};

use axum::{
    Json, Router,
    extract::{Path, State},
    response::{Html, IntoResponse},
    routing::{get, post},
};
use chrono::Utc;
use tokio::sync::{Notify, RwLock};

use crate::{
    config::WebexConfig,
    ssh::list_files,
    state::{AppState, Job},
};

const INDEX_FILE: &str = include_str!("../assets/index.html");

async fn index(State(cfg): State<Arc<WebexConfig>>) -> impl IntoResponse {
    let index = if let Some(path) = cfg.serve.index_path.as_deref() {
        std::fs::read_to_string(path).unwrap_or(INDEX_FILE.to_string())
    } else {
        INDEX_FILE.to_string()
    };
    Html(index)
}

async fn get_path(
    State(cfg): State<Arc<WebexConfig>>,
    path: Option<Path<String>>,
) -> impl IntoResponse {
    let mut ret = Vec::new();
    let files = list_files(cfg.as_ref(), &path.map(|x| x.0).unwrap_or("/".into())).unwrap();
    for file in files {
        let file_name = file.name().unwrap_or_default();
        if file_name == "." || file_name == ".." || file_name.is_empty() {
            continue;
        }
        let mut file_info = HashMap::new();
        file_info.insert("name", file.name().unwrap_or_default().into());
        file_info.insert("size", file.len().unwrap_or_default().to_string());
        file_info.insert(
            "type",
            format!(
                "{:?}",
                file.file_type().unwrap_or(libssh_rs::FileType::Unknown)
            ),
        );
        ret.push(file_info);
    }

    Json(ret)
}

async fn create_job(
    State(notify): State<Arc<Notify>>,
    State(jobs): State<Arc<RwLock<Vec<Job>>>>,
    text: String,
) -> impl IntoResponse {
    let mut write = jobs.write().await;
    write.push(Job {
        src: text.clone(),
        dst: text.clone(),
        size: 10,
        current: 0,
        created: Utc::now(),
    });
    notify.notify_one();
    text
}

async fn get_current(State(current): State<Arc<RwLock<Option<Job>>>>) -> impl IntoResponse {
    match current.read().await.as_ref() {
        Some(job) => Json(job).into_response(),
        None => Json(None::<String>).into_response(),
    }
}

async fn list_jobs(State(current): State<Arc<RwLock<Vec<Job>>>>) -> impl IntoResponse {
    Json(&*current.read().await).into_response()
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/list/{*rest}", get(get_path))
        .route("/list", get(get_path))
        .route("/job", get(get_current))
        .route("/job", post(create_job))
        .route("/jobs", get(list_jobs))
}

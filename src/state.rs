use std::sync::Arc;

use axum::extract::FromRef;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tokio::sync::{Notify, RwLock};

use crate::config::WebexConfig;

#[derive(Serialize)]
pub struct Job {
    pub src: String,
    pub dst: String,
    pub size: usize,
    pub current: usize,
    pub created: DateTime<Utc>,
}

#[derive(FromRef, Clone)]
pub struct AppState {
    pub config: Arc<WebexConfig>,
    pub jobs: Arc<RwLock<Vec<Job>>>,
    pub notify: Arc<Notify>,
    pub current: Arc<RwLock<Option<Job>>>,
}

use std::{sync::Arc, thread};

use axum::middleware;
use tokio::sync::{Notify, RwLock};

use crate::{
    auth::auth_layer,
    config::WebexConfig,
    ssh::{CopyFileEvent, copy_file},
    state::{AppState, Job},
};

mod auth;
mod config;
mod routes;
mod ssh;
mod state;

#[tokio::main]
async fn main() {
    let config_path = std::env::var("SSH_WEBEX_CONFIG").unwrap_or("config.toml".into());
    let config_str = std::fs::read_to_string(config_path).unwrap_or_default();
    let config: WebexConfig = toml::from_str(&config_str).unwrap();
    let bind_addr: String = config
        .serve
        .bind
        .as_deref()
        .unwrap_or("0.0.0.0:3000")
        .into();

    let jobs: Arc<RwLock<Vec<Job>>> = Arc::default();
    let notify: Arc<Notify> = Arc::default();

    let state = AppState {
        config: Arc::new(config),
        jobs: jobs.clone(),
        notify: notify.clone(),
        current: Arc::default(),
    };

    tokio::spawn(copier(state.clone()));

    let app = routes::router()
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth_layer));

    println!("Listening on {bind_addr}");
    let listener = tokio::net::TcpListener::bind(bind_addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn copier(
    AppState {
        config,
        jobs,
        notify,
        current,
    }: AppState,
) -> ! {
    loop {
        notify.notified().await;
        loop {
            let maybe_job = { jobs.write().await.pop() };
            if let Some(job) = maybe_job {
                let src = job.src.clone();
                let dst = job.dst.clone();

                *current.write().await = Some(job);
                let config = config.clone();
                let (tx, mut rx) = tokio::sync::mpsc::channel(16);

                let handler = {
                    let current = current.clone();
                    move |x| match x {
                        CopyFileEvent::Start(size) => {
                            if let Some(job) = current.blocking_write().as_mut() {
                                job.size = size
                            }
                        }
                        CopyFileEvent::Written(written) => tx.blocking_send(written).unwrap(),
                    }
                };

                thread::spawn(move || copy_file(config.as_ref(), &src, &dst, handler));

                while let Some(written) = rx.recv().await {
                    update_written(current.as_ref(), written).await
                }
                *current.write().await = None;
            } else {
                break;
            }
        }
    }
}

async fn update_written(current: &RwLock<Option<Job>>, written: usize) {
    let mut guard = current.write().await;
    if let Some(job) = guard.as_mut() {
        job.current = job.current + written;
    }
}

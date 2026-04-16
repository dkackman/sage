use std::io;
use std::sync::Arc;

use futures::{stream::FuturesUnordered, StreamExt};
use tauri::{command, Emitter, State};
use tokio::sync::Semaphore;
use uuid::Uuid;

use crate::{
    app_state::AppState,
    apps::{
        bridge::fetch::bridge_fetch_http_inner,
        types::{SageBridgeFetchBatchRequest, SageBridgeFetchResponse},
    },
    error::Result,
};
use crate::apps::limits::MAX_BRIDGE_BATCH_REQUESTS;

#[command]
#[specta::specta]
pub async fn bridge_fetch_http_batch(
    state: State<'_, AppState>,
    app_id: String,
    req: SageBridgeFetchBatchRequest,
) -> Result<Vec<SageBridgeFetchResponse>> {
    if req.requests.len() > MAX_BRIDGE_BATCH_REQUESTS {
        return Err(io::Error::other(format!(
            "batch request count {} exceeds limit {}",
            req.requests.len(),
            MAX_BRIDGE_BATCH_REQUESTS
        )).into());
    }
    let app_state = state.inner().clone();
    let max = req.max_concurrency.unwrap_or(8).max(1);
    let semaphore = Arc::new(Semaphore::new(max));
    let request_count = req.requests.len();
    let mut futures = FuturesUnordered::new();

    for (index, request) in req.requests.into_iter().enumerate() {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|err| io::Error::other(format!("failed to acquire batch semaphore: {err}")))?;
        let app_state = app_state.clone();
        let app_id = app_id.clone();

        futures.push(async move {
            let _permit = permit;
            let result = bridge_fetch_http_inner(app_state, app_id, request).await;
            (index, result)
        });
    }

    let mut ordered: Vec<Option<SageBridgeFetchResponse>> = vec![None; request_count];

    while let Some((index, result)) = futures.next().await {
        ordered[index] = Some(result?);
    }

    let mut out = Vec::with_capacity(request_count);
    for item in ordered {
        let value = item.ok_or_else(|| io::Error::other("missing batch result"))?;
        out.push(value);
    }
    Ok(out)
}

#[command]
#[specta::specta]
pub async fn bridge_fetch_http_batch_stream(
    app_handle: tauri::AppHandle,
    state: State<'_, AppState>,
    app_id: String,
    source_label: String,
    req: SageBridgeFetchBatchRequest,
) -> Result<String> {
    if req.requests.len() > MAX_BRIDGE_BATCH_REQUESTS {
        return Err(io::Error::other(format!(
            "batch request count {} exceeds limit {}",
            req.requests.len(),
            MAX_BRIDGE_BATCH_REQUESTS
        )).into());
    }
    let batch_id = format!("batch-{}", Uuid::new_v4());
    let batch_id_for_spawn = batch_id.clone();
    let batch_id_done = batch_id.clone();
    let app_state = state.inner().clone();
    let max = req.max_concurrency.unwrap_or(8).max(1);
    let semaphore = Arc::new(Semaphore::new(max));

    tauri::async_runtime::spawn(async move {
        let mut futures = FuturesUnordered::new();

        for (index, request) in req.requests.into_iter().enumerate() {
            let permit = match semaphore.clone().acquire_owned().await {
                Ok(permit) => permit,
                Err(_) => break,
            };

            let app_handle = app_handle.clone();
            let app_state = app_state.clone();
            let app_id = app_id.clone();
            let source_label = source_label.clone();
            let batch_id = batch_id_for_spawn.clone();

            futures.push(async move {
                let _permit = permit;

                let result = bridge_fetch_http_inner(app_state, app_id, request).await;

                let payload = match result {
                    Ok(res) => serde_json::json!({
                        "batchId": batch_id,
                        "index": index,
                        "ok": true,
                        "result": res
                    }),
                    Err(err) => serde_json::json!({
                        "batchId": batch_id,
                        "index": index,
                        "ok": false,
                        "error": err.to_string()
                    }),
                };
                let _ = app_handle.emit_to(&source_label, "sage-bridge:batch:result", payload);
            });
        }

        while futures.next().await.is_some() {}
        let _ = app_handle.emit_to(
            &source_label,
            "sage-bridge:batch:done",
            serde_json::json!({
                "batchId": batch_id_done
            }),
        );
    });

    Ok(batch_id)
}

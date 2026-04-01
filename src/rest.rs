// rest.rs — REST + OpenAPI routes for fs-tasks.

use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

use crate::controller::TaskController;
use crate::model::TaskPipeline;

// ── OpenAPI doc ───────────────────────────────────────────────────────────────

#[allow(clippy::needless_for_each)] // triggered by utoipa macro internals
#[derive(OpenApi)]
#[openapi(
    paths(list_tasks, create_task, delete_task, toggle_task),
    components(schemas(TaskPipeline, CreateTaskBody, ToggleTaskResult))
)]
pub struct ApiDoc;

// ── Request / Response types ──────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateTaskBody {
    pub name: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ToggleTaskResult {
    pub ok: bool,
    pub enabled: bool,
}

// ── Router ────────────────────────────────────────────────────────────────────

/// Build the axum router for the tasks REST API.
pub fn router(ctrl: TaskController) -> Router {
    Router::new()
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/{id}", delete(delete_task))
        .route("/tasks/{id}/toggle", post(toggle_task))
        .with_state(ctrl)
}

// ── Handlers ──────────────────────────────────────────────────────────────────

/// List all task pipelines.
#[utoipa::path(get, path = "/tasks", responses((status = 200, body = Vec<TaskPipeline>)))]
async fn list_tasks(State(ctrl): State<TaskController>) -> Json<Vec<TaskPipeline>> {
    Json(ctrl.list())
}

/// Create a new task pipeline.
#[utoipa::path(
    post,
    path = "/tasks",
    request_body = CreateTaskBody,
    responses((status = 201, body = TaskPipeline))
)]
async fn create_task(
    State(ctrl): State<TaskController>,
    Json(body): Json<CreateTaskBody>,
) -> (StatusCode, Json<TaskPipeline>) {
    let task = ctrl.create(body.name);
    (StatusCode::CREATED, Json(task))
}

/// Delete a task pipeline by id.
#[utoipa::path(
    delete,
    path = "/tasks/{id}",
    params(("id" = String, Path, description = "Task id")),
    responses((status = 204), (status = 404))
)]
async fn delete_task(State(ctrl): State<TaskController>, Path(id): Path<String>) -> StatusCode {
    if ctrl.delete(&id) {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}

/// Toggle a task pipeline enabled/disabled.
#[utoipa::path(
    post,
    path = "/tasks/{id}/toggle",
    params(("id" = String, Path, description = "Task id")),
    responses((status = 200, body = ToggleTaskResult), (status = 404))
)]
async fn toggle_task(
    State(ctrl): State<TaskController>,
    Path(id): Path<String>,
) -> (StatusCode, Json<ToggleTaskResult>) {
    match ctrl.toggle(&id) {
        Some(enabled) => (StatusCode::OK, Json(ToggleTaskResult { ok: true, enabled })),
        None => (
            StatusCode::NOT_FOUND,
            Json(ToggleTaskResult {
                ok: false,
                enabled: false,
            }),
        ),
    }
}

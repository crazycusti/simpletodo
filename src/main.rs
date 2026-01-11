use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use axum::{
    extract::{Form, Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;

mod db;
mod models;

use db::Database;
use models::Todo;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
}

#[derive(Deserialize)]
struct AddForm {
    title: String,
    description: Option<String>,
    deadline: Option<String>,
}

#[derive(Deserialize)]
struct UpdateForm {
    id: i64,
    description: Option<String>,
    deadline: Option<String>,
}

#[derive(Deserialize)]
struct IdForm {
    id: i64,
}

#[derive(Deserialize)]
struct SubtaskForm {
    todo_id: i64,
    title: String,
}

#[derive(Deserialize)]
struct ToggleSubtaskForm {
    id: i64,
    todo_id: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = PathBuf::from("todo.db");
    let state = AppState { db_path };

    let app = Router::new()
        .route("/", get(index))
        .route("/todo/:id", get(todo_detail))
        .route("/add", post(add_todo))
        .route("/update", post(update_todo))
        .route("/add-subtask", post(add_subtask))
        .route("/toggle-subtask", post(toggle_subtask))
        .route("/complete", post(complete_todo))
        .route("/delete", post(delete_todo))
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], 5876));
    println!("simpletodo running on http://{addr}");

    axum::serve(tokio::net::TcpListener::bind(addr).await?, app).await?;

    Ok(())
}

async fn index(State(state): State<AppState>) -> Result<Html<String>, StatusCode> {
    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let todos = db.list_todos().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut body = String::new();
    body.push_str(page_start());
    body.push_str(add_todo_form());
    body.push_str("<div class=\"todo-list\">");

    if todos.is_empty() {
        body.push_str("<div class=\"subtitle\">Noch keine Todos. Leg los!</div>");
    } else {
        for todo in todos {
            body.push_str(&render_todo_card(&todo));
        }
    }

    body.push_str("</div>");
    body.push_str(page_end());

    Ok(Html(body))
}

async fn todo_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Html<String>, StatusCode> {
    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let todo = db.get_todo(id).map_err(|_| StatusCode::NOT_FOUND)?;

    let mut body = String::new();
    body.push_str(page_start());
    body.push_str("<a class=\"link\" href=\"/\">← Zurück</a>");
    body.push_str(&render_todo_detail(&todo));
    body.push_str(page_end());

    Ok(Html(body))
}

async fn add_todo(
    State(state): State<AppState>,
    Form(form): Form<AddForm>,
) -> Result<impl IntoResponse, StatusCode> {
    if form.title.trim().is_empty() {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }

    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.add_todo(
        form.title.trim(),
        form.description.as_deref().map(|value| value.trim()).filter(|v| !v.is_empty()),
        form.deadline.as_deref().map(|value| value.trim()).filter(|v| !v.is_empty()),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_home())
}

async fn update_todo(
    State(state): State<AppState>,
    Form(form): Form<UpdateForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.update_todo(
        form.id,
        form.description.as_deref().map(|value| value.trim()).filter(|v| !v.is_empty()),
        form.deadline.as_deref().map(|value| value.trim()).filter(|v| !v.is_empty()),
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_to(&format!("/todo/{}", form.id)))
}

async fn add_subtask(
    State(state): State<AppState>,
    Form(form): Form<SubtaskForm>,
) -> Result<impl IntoResponse, StatusCode> {
    if form.title.trim().is_empty() {
        return Ok(StatusCode::BAD_REQUEST.into_response());
    }

    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.add_subtask(form.todo_id, form.title.trim())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_to(&format!("/todo/{}", form.todo_id)))
}

async fn toggle_subtask(
    State(state): State<AppState>,
    Form(form): Form<ToggleSubtaskForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.toggle_subtask(form.id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_to(&format!("/todo/{}", form.todo_id)))
}

async fn complete_todo(
    State(state): State<AppState>,
    Form(form): Form<IdForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.complete_todo(form.id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_home())
}

async fn delete_todo(
    State(state): State<AppState>,
    Form(form): Form<IdForm>,
) -> Result<impl IntoResponse, StatusCode> {
    let db = Database::connect(&state.db_path).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    db.delete_todo(form.id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_home())
}

fn render_todo_card(todo: &Todo) -> String {
    let status_class = if todo.completed_at.is_some() { "status done" } else { "status" };
    let status_label = if todo.completed_at.is_some() { "Erledigt" } else { "Offen" };
    let created = todo.created_at.format("%d.%m.%Y %H:%M");
    let deadline = todo
        .deadline
        .as_deref()
        .map(|value| format!("<div class=\"deadline\">Deadline: {}</div>", html_escape(value)))
        .unwrap_or_default();
    let description = todo
        .description
        .as_deref()
        .map(|value| format!("<div class=\"description\">{}</div>", html_escape(value)))
        .unwrap_or_default();
    let progress = progress_percent(todo.subtask_done, todo.subtask_total);

    let mut body = String::new();
    body.push_str(&format!(
        r#"<div class="todo">
  <div class="meta">
    <div class="title">{title}</div>
    {description}
    <div class="time">Erstellt am {created}</div>
    {deadline}
    <div class="progress">
      <div class="progress-bar" style="width: {progress}%"></div>
    </div>
    <div class="progress-label">{done} / {total} erledigt ({progress}%)</div>
  </div>
  <div class="actions">
    <span class="{status_class}">{status_label}</span>
    <a class="button ghost" href="/todo/{id}">Konfigurieren</a>
"#,
        title = html_escape(&todo.title),
        description = description,
        created = created,
        deadline = deadline,
        progress = progress,
        done = todo.subtask_done,
        total = todo.subtask_total,
        status_class = status_class,
        status_label = status_label,
        id = todo.id
    ));

    if todo.completed_at.is_none() {
        body.push_str(&format!(
            r#"<form method="post" action="/complete">
  <input type="hidden" name="id" value="{id}" />
  <button type="submit">Done</button>
</form>"#,
            id = todo.id
        ));
    }

    body.push_str(&format!(
        r#"<form method="post" action="/delete">
  <input type="hidden" name="id" value="{id}" />
  <button class="delete" type="submit">Löschen</button>
</form>
  </div>
</div>"#,
        id = todo.id
    ));

    body
}

fn render_todo_detail(todo: &Todo) -> String {
    let deadline_value = todo.deadline.as_deref().unwrap_or("");
    let description_value = todo.description.as_deref().unwrap_or("");
    let progress = progress_percent(todo.subtask_done, todo.subtask_total);

    let mut body = String::new();
    body.push_str(&format!(
        r#"<div class="detail">
  <div class="detail-header">
    <h2>{title}</h2>
    <div class="progress">
      <div class="progress-bar" style="width: {progress}%"></div>
    </div>
    <div class="progress-label">{done} / {total} erledigt ({progress}%)</div>
  </div>
  <form method="post" action="/update" class="stack">
    <input type="hidden" name="id" value="{id}" />
    <label>
      Beschreibung
      <textarea name="description" rows="3" placeholder="Beschreibung">{description}</textarea>
    </label>
    <label>
      Deadline (Tag)
      <input type="date" name="deadline" value="{deadline}" />
    </label>
    <button type="submit">Speichern</button>
  </form>
  <div class="subtasks">
    <h3>Einzelaufgaben</h3>
    <form method="post" action="/add-subtask" class="row">
      <input type="hidden" name="todo_id" value="{id}" />
      <input type="text" name="title" placeholder="Neue Aufgabe" required />
      <button type="submit">Hinzufügen</button>
    </form>
    <div class="subtask-list">
"#,
        title = html_escape(&todo.title),
        progress = progress,
        done = todo.subtask_done,
        total = todo.subtask_total,
        description = html_escape(description_value),
        deadline = html_escape(deadline_value),
        id = todo.id
    ));

    if todo.subtasks.is_empty() {
        body.push_str("<div class=\"subtitle\">Noch keine Einzelaufgaben.</div>");
    } else {
        for subtask in &todo.subtasks {
            let checked = if subtask.is_done { "checked" } else { "" };
            let status = if subtask.is_done { "done" } else { "open" };
            body.push_str(&format!(
                r#"<div class="subtask {status}">
  <form method="post" action="/toggle-subtask">
    <input type="hidden" name="id" value="{id}" />
    <input type="hidden" name="todo_id" value="{todo_id}" />
    <label class="checkbox">
      <input type="checkbox" onchange="this.form.submit()" {checked} />
      <span>{title}</span>
    </label>
  </form>
</div>"#,
                status = status,
                id = subtask.id,
                todo_id = subtask.todo_id,
                checked = checked,
                title = html_escape(&subtask.title)
            ));
        }
    }

    body.push_str(
        r#"    </div>
  </div>
</div>"#,
    );

    body
}

fn add_todo_form() -> &'static str {
    r#"<form method="post" action="/add" class="stack">
  <label>
    Titel
    <input type="text" name="title" placeholder="Neues Todo" required />
  </label>
  <label>
    Beschreibung
    <textarea name="description" rows="2" placeholder="Optional"></textarea>
  </label>
  <label>
    Deadline (Tag)
    <input type="date" name="deadline" />
  </label>
  <button type="submit">Hinzufügen</button>
</form>"#
}

fn page_start() -> &'static str {
    r#"<!doctype html>
<html lang="de">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>simpletodo</title>
  <style>
    :root {
      color-scheme: light;
      font-family: "Inter", system-ui, -apple-system, sans-serif;
      background: #f4f5f7;
    }
    body {
      margin: 0;
      padding: 32px;
      display: flex;
      justify-content: center;
    }
    .app {
      width: min(860px, 100%);
      background: #ffffff;
      border-radius: 16px;
      box-shadow: 0 24px 48px rgba(15, 23, 42, 0.08);
      padding: 28px;
    }
    h1 {
      margin: 0 0 16px 0;
      font-size: 28px;
      letter-spacing: -0.02em;
    }
    h2 {
      margin: 0 0 8px 0;
    }
    h3 {
      margin: 0 0 12px 0;
    }
    .subtitle {
      color: #64748b;
      margin-bottom: 12px;
    }
    form {
      display: flex;
      gap: 12px;
      align-items: flex-end;
    }
    form.stack {
      flex-direction: column;
      align-items: stretch;
      margin-bottom: 24px;
    }
    form.row {
      margin-bottom: 16px;
    }
    label {
      display: flex;
      flex-direction: column;
      gap: 6px;
      font-size: 13px;
      color: #475569;
    }
    input[type="text"],
    input[type="date"],
    textarea {
      width: 100%;
      padding: 12px 14px;
      border-radius: 10px;
      border: 1px solid #e2e8f0;
      font-size: 15px;
      font-family: inherit;
    }
    textarea {
      resize: vertical;
    }
    button,
    .button {
      border: none;
      border-radius: 10px;
      padding: 12px 16px;
      background: #111827;
      color: white;
      font-weight: 600;
      cursor: pointer;
      text-decoration: none;
      text-align: center;
    }
    .button.ghost {
      background: #e2e8f0;
      color: #0f172a;
    }
    .todo-list {
      display: grid;
      gap: 12px;
    }
    .todo,
    .detail {
      display: flex;
      flex-direction: column;
      gap: 16px;
      padding: 16px;
      border-radius: 12px;
      background: #f8fafc;
      border: 1px solid #e2e8f0;
    }
    .todo .meta {
      display: flex;
      flex-direction: column;
      gap: 6px;
    }
    .todo .title {
      font-weight: 600;
      font-size: 18px;
    }
    .description {
      color: #475569;
    }
    .deadline {
      font-size: 13px;
      color: #0f172a;
    }
    .time {
      font-size: 12px;
      color: #94a3b8;
    }
    .status {
      font-size: 12px;
      text-transform: uppercase;
      letter-spacing: 0.08em;
      color: #0f172a;
      background: #e2e8f0;
      padding: 4px 8px;
      border-radius: 999px;
    }
    .status.done {
      background: #dcfce7;
      color: #166534;
    }
    .actions {
      display: flex;
      gap: 8px;
      flex-wrap: wrap;
    }
    .actions button {
      background: #e2e8f0;
      color: #0f172a;
      font-weight: 600;
      padding: 8px 12px;
    }
    .actions button.delete {
      background: #fee2e2;
      color: #991b1b;
    }
    .progress {
      height: 8px;
      background: #e2e8f0;
      border-radius: 999px;
      overflow: hidden;
    }
    .progress-bar {
      height: 100%;
      background: #0ea5e9;
    }
    .progress-label {
      font-size: 12px;
      color: #475569;
    }
    .link {
      display: inline-block;
      margin-bottom: 16px;
      color: #0f172a;
      text-decoration: none;
      font-weight: 600;
    }
    .subtasks {
      display: flex;
      flex-direction: column;
      gap: 12px;
    }
    .subtask-list {
      display: grid;
      gap: 8px;
    }
    .subtask {
      padding: 10px 12px;
      border-radius: 10px;
      border: 1px solid #e2e8f0;
      background: #ffffff;
    }
    .subtask.done {
      background: #f0fdf4;
    }
    .checkbox {
      display: flex;
      align-items: center;
      gap: 8px;
      font-size: 14px;
      color: #0f172a;
    }
  </style>
</head>
<body>
  <div class="app">
    <h1>simpletodo</h1>
    <div class="subtitle">Ein minimaler Todo-Tracker mit SQLite.</div>
"#
}

fn page_end() -> &'static str {
    r#"  </div>
</body>
</html>"#
}

fn redirect_home() -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, "/")]).into_response()
}

fn redirect_to(path: &str) -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, path)]).into_response()
}

fn progress_percent(done: usize, total: usize) -> usize {
    if total == 0 {
        0
    } else {
        (done * 100) / total
    }
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

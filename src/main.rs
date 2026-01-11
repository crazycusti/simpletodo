use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use axum::{
    extract::{Form, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;

mod db;
mod models;

use db::Database;

#[derive(Clone)]
struct AppState {
    db_path: PathBuf,
}

#[derive(Deserialize)]
struct AddForm {
    title: String,
}

#[derive(Deserialize)]
struct IdForm {
    id: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let db_path = PathBuf::from("todo.db");
    let state = AppState { db_path };

    let app = Router::new()
        .route("/", get(index))
        .route("/add", post(add_todo))
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
    let todos = db.list_todos(true).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut body = String::new();
    body.push_str(
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
      width: min(720px, 100%);
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
    .subtitle {
      color: #64748b;
      margin-bottom: 24px;
    }
    form {
      display: flex;
      gap: 12px;
      margin-bottom: 24px;
    }
    input[type="text"] {
      flex: 1;
      padding: 12px 14px;
      border-radius: 10px;
      border: 1px solid #e2e8f0;
      font-size: 15px;
    }
    button {
      border: none;
      border-radius: 10px;
      padding: 12px 16px;
      background: #111827;
      color: white;
      font-weight: 600;
      cursor: pointer;
    }
    .todo-list {
      display: grid;
      gap: 12px;
    }
    .todo {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 12px 16px;
      border-radius: 12px;
      background: #f8fafc;
      border: 1px solid #e2e8f0;
    }
    .todo .meta {
      display: flex;
      flex-direction: column;
      gap: 4px;
    }
    .todo .title {
      font-weight: 600;
    }
    .todo .time {
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
  </style>
</head>
<body>
  <div class="app">
    <h1>simpletodo</h1>
    <div class="subtitle">Ein minimaler Todo-Tracker mit SQLite.</div>
    <form method="post" action="/add">
      <input type="text" name="title" placeholder="Neues Todo" required />
      <button type="submit">Hinzufügen</button>
    </form>
    <div class="todo-list">
"#,
    );

    if todos.is_empty() {
        body.push_str("<div class=\"subtitle\">Noch keine Todos. Leg los!</div>");
    } else {
        for todo in todos {
            let status_class = if todo.completed_at.is_some() { "status done" } else { "status" };
            let status_label = if todo.completed_at.is_some() { "Erledigt" } else { "Offen" };
            let created = todo.created_at.format("%d.%m.%Y %H:%M");
            body.push_str(&format!(
                r#"<div class="todo">
  <div class="meta">
    <div class="title">{title}</div>
    <div class="time">Erstellt am {created}</div>
  </div>
  <div class="actions">
    <span class="{status_class}">{status_label}</span>
"#,
                title = html_escape(&todo.title),
                created = created,
                status_class = status_class,
                status_label = status_label
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
        }
    }

    body.push_str(
        r#"    </div>
  </div>
</body>
</html>"#,
    );

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
    db.add_todo(form.title.trim())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(redirect_home())
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

fn redirect_home() -> Response {
    (StatusCode::SEE_OTHER, [(header::LOCATION, "/")]).into_response()
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

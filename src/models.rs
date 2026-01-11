use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub description: Option<String>,
    pub deadline: Option<String>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub subtasks: Vec<Subtask>,
    pub subtask_total: usize,
    pub subtask_done: usize,
}

#[derive(Debug, Serialize)]
pub struct Subtask {
    pub id: i64,
    pub todo_id: i64,
    pub title: String,
    pub is_done: bool,
}

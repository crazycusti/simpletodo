use std::path::Path;

use anyhow::Context;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::models::Todo;

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn connect<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let conn = Connection::open(path.as_ref())
            .with_context(|| format!("opening database at {}", path.as_ref().display()))?;
        let db = Self { conn };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> anyhow::Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS todos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL,
                created_at TEXT NOT NULL,
                completed_at TEXT
            );
            "#,
        )?;
        Ok(())
    }

    pub fn add_todo(&self, title: &str) -> anyhow::Result<Todo> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT INTO todos (title, created_at) VALUES (?1, ?2)",
            params![title, now.to_rfc3339()],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Todo {
            id,
            title: title.to_string(),
            created_at: now,
            completed_at: None,
        })
    }

    pub fn list_todos(&self, include_completed: bool) -> anyhow::Result<Vec<Todo>> {
        let mut stmt = if include_completed {
            self.conn.prepare(
                "SELECT id, title, created_at, completed_at FROM todos ORDER BY id DESC",
            )?
        } else {
            self.conn.prepare(
                "SELECT id, title, created_at, completed_at FROM todos WHERE completed_at IS NULL ORDER BY id DESC",
            )?
        };

        let rows = stmt.query_map([], |row| {
            let created_at: String = row.get(2)?;
            let completed_at: Option<String> = row.get(3)?;
            Ok(Todo {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: parse_datetime(&created_at),
                completed_at: completed_at.map(|value| parse_datetime(&value)),
            })
        })?;

        let mut todos = Vec::new();
        for todo in rows {
            todos.push(todo?);
        }
        Ok(todos)
    }

    pub fn complete_todo(&self, id: i64) -> anyhow::Result<()> {
        let now = Utc::now().to_rfc3339();
        let updated = self.conn.execute(
            "UPDATE todos SET completed_at = ?1 WHERE id = ?2 AND completed_at IS NULL",
            params![now, id],
        )?;
        if updated == 0 {
            anyhow::bail!("todo {id} not found or already completed");
        }
        Ok(())
    }

    pub fn delete_todo(&self, id: i64) -> anyhow::Result<()> {
        let deleted = self
            .conn
            .execute("DELETE FROM todos WHERE id = ?1", params![id])?;
        if deleted == 0 {
            anyhow::bail!("todo {id} not found");
        }
        Ok(())
    }
}

fn parse_datetime(value: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

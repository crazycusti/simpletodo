use std::path::Path;

use anyhow::Context;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::models::{Subtask, Todo};

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

            CREATE TABLE IF NOT EXISTS subtasks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                todo_id INTEGER NOT NULL,
                title TEXT NOT NULL,
                is_done INTEGER NOT NULL DEFAULT 0,
                FOREIGN KEY(todo_id) REFERENCES todos(id) ON DELETE CASCADE
            );
            "#,
        )?;

        self.ensure_column("todos", "description", "TEXT")?;
        self.ensure_column("todos", "deadline", "TEXT")?;

        Ok(())
    }

    fn ensure_column(&self, table: &str, column: &str, column_type: &str) -> anyhow::Result<()> {
        let mut stmt = self
            .conn
            .prepare("SELECT name FROM pragma_table_info(?1)")?;
        let mut rows = stmt.query([table])?;
        while let Some(row) = rows.next()? {
            let name: String = row.get(0)?;
            if name == column {
                return Ok(());
            }
        }

        let sql = format!("ALTER TABLE {table} ADD COLUMN {column} {column_type}");
        self.conn.execute(&sql, [])?;
        Ok(())
    }

    pub fn add_todo(
        &self,
        title: &str,
        description: Option<&str>,
        deadline: Option<&str>,
    ) -> anyhow::Result<Todo> {
        let now = Utc::now();
        self.conn.execute(
            "INSERT INTO todos (title, description, deadline, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![title, description, deadline, now.to_rfc3339()],
        )?;
        let id = self.conn.last_insert_rowid();
        Ok(Todo {
            id,
            title: title.to_string(),
            description: description.map(|value| value.to_string()),
            deadline: deadline.map(|value| value.to_string()),
            created_at: now,
            completed_at: None,
            subtasks: Vec::new(),
            subtask_total: 0,
            subtask_done: 0,
        })
    }

    pub fn update_todo(
        &self,
        id: i64,
        description: Option<&str>,
        deadline: Option<&str>,
    ) -> anyhow::Result<()> {
        let updated = self.conn.execute(
            "UPDATE todos SET description = ?1, deadline = ?2 WHERE id = ?3",
            params![description, deadline, id],
        )?;
        if updated == 0 {
            anyhow::bail!("todo {id} not found");
        }
        Ok(())
    }

    pub fn list_todos(&self) -> anyhow::Result<Vec<Todo>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, deadline, created_at, completed_at FROM todos ORDER BY id DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let created_at: String = row.get(4)?;
            let completed_at: Option<String> = row.get(5)?;
            Ok(Todo {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                deadline: row.get(3)?,
                created_at: parse_datetime(&created_at),
                completed_at: completed_at.map(|value| parse_datetime(&value)),
                subtasks: Vec::new(),
                subtask_total: 0,
                subtask_done: 0,
            })
        })?;

        let mut todos = Vec::new();
        for todo in rows {
            let mut todo = todo?;
            todo.subtasks = self.list_subtasks(todo.id)?;
            let (done, total) = self.subtask_counts(todo.id)?;
            todo.subtask_total = total;
            todo.subtask_done = done;
            todos.push(todo);
        }
        Ok(todos)
    }

    pub fn get_todo(&self, id: i64) -> anyhow::Result<Todo> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, description, deadline, created_at, completed_at FROM todos WHERE id = ?1",
        )?;
        let todo = stmt.query_row([id], |row| {
            let created_at: String = row.get(4)?;
            let completed_at: Option<String> = row.get(5)?;
            Ok(Todo {
                id: row.get(0)?,
                title: row.get(1)?,
                description: row.get(2)?,
                deadline: row.get(3)?,
                created_at: parse_datetime(&created_at),
                completed_at: completed_at.map(|value| parse_datetime(&value)),
                subtasks: Vec::new(),
                subtask_total: 0,
                subtask_done: 0,
            })
        })?;

        let mut todo = todo;
        todo.subtasks = self.list_subtasks(todo.id)?;
        let (done, total) = self.subtask_counts(todo.id)?;
        todo.subtask_total = total;
        todo.subtask_done = done;
        Ok(todo)
    }

    pub fn add_subtask(&self, todo_id: i64, title: &str) -> anyhow::Result<()> {
        self.conn.execute(
            "INSERT INTO subtasks (todo_id, title) VALUES (?1, ?2)",
            params![todo_id, title],
        )?;
        Ok(())
    }

    pub fn toggle_subtask(&self, id: i64) -> anyhow::Result<()> {
        self.conn.execute(
            "UPDATE subtasks SET is_done = CASE WHEN is_done = 1 THEN 0 ELSE 1 END WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn list_subtasks(&self, todo_id: i64) -> anyhow::Result<Vec<Subtask>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, todo_id, title, is_done FROM subtasks WHERE todo_id = ?1 ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([todo_id], |row| {
            let is_done: i64 = row.get(3)?;
            Ok(Subtask {
                id: row.get(0)?,
                todo_id: row.get(1)?,
                title: row.get(2)?,
                is_done: is_done == 1,
            })
        })?;

        let mut subtasks = Vec::new();
        for subtask in rows {
            subtasks.push(subtask?);
        }
        Ok(subtasks)
    }

    fn subtask_counts(&self, todo_id: i64) -> anyhow::Result<(usize, usize)> {
        let mut stmt = self.conn.prepare(
            "SELECT SUM(CASE WHEN is_done = 1 THEN 1 ELSE 0 END) as done, COUNT(*) as total FROM subtasks WHERE todo_id = ?1",
        )?;
        let (done, total): (Option<i64>, i64) = stmt.query_row([todo_id], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })?;
        Ok((done.unwrap_or(0) as usize, total as usize))
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

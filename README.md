# simpletodo

A lightweight, standalone todo web app built in Rust with a bundled SQLite database.

## Quick start

```bash
cargo run
```

Open http://localhost:5876 to use the app.

The database is stored in `todo.db` by default. Each todo can include a description,
optional deadline, and subtask checklist with a progress bar.

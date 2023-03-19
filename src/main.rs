use axum::{routing::get, Form, routing::post, Extension, Router, Server};
use sqlx::{Sqlite, sqlite::SqlitePool};
use std::sync::{Arc};
use axum::response::{Html, IntoResponse, Redirect};
use sqlx::migrate::MigrateDatabase;
use serde::Deserialize;

#[derive(Debug)]
struct AppState {
    db_pool: SqlitePool,
}

#[derive(sqlx::FromRow)]
struct Message {
    id: i32,
    name: String,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[axum_macros::debug_handler]
async fn show_messages_handler(state: Extension<Arc<AppState>>) -> impl IntoResponse {
    let mut conn = state.db_pool.acquire().await.unwrap();
    let messages = sqlx::query_as::<_, Message>("SELECT * FROM messages ORDER BY created_at DESC")
        .fetch_all(&mut conn)
        .await
        .unwrap();

    let message_list = messages.iter().enumerate().fold(String::new(), |acc, (_i, msg)| {
        acc + &format!("{}. {} - {} [{}]\n", msg.id, msg.name, msg.content, msg.created_at)
    });

    Html(format!(
        r#"
            <html>
            <head>
                <title>Message Board</title>
            </head>
            <body>

                <form method="POST" action="/add">
                <input type="text" name="name" />
                <textarea name="message"></textarea>
                <input type="submit" value="Save" />
                </form>

                <h1>Messages:</h1>
                <pre>{}</pre>

            </body>
            </html>
        "#,
        message_list
    ))
}


#[derive(Deserialize)]
struct MessageForm {
    name: String,
    message: String,
}

#[axum_macros::debug_handler]
async fn add_message_handler( Extension(state): Extension<Arc<AppState>>, Form(message_form): Form<MessageForm>) -> impl IntoResponse {
    let mut conn = state.db_pool.acquire().await.unwrap();
    sqlx::query("INSERT INTO messages (name, content) VALUES (?, ?)")
        .bind(&message_form.name)
        .bind(&message_form.message)
        .execute(&mut conn)
        .await
        .unwrap();

    Redirect::to("/")
}

#[tokio::main]
async fn main() {
    let db_url = String::from("sqlite://messages.db");

    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        Sqlite::create_database(&db_url).await.unwrap();
    }

    let instances = SqlitePool::connect(&db_url).await.unwrap();

    let app_state = Arc::new(AppState {
        db_pool: instances.clone(),
    });

    sqlx::query(
        r#"
            CREATE TABLE IF NOT EXISTS messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        "#,
    )
        .execute(&instances)
        .await
        .unwrap();

    let app = Router::new()
        .route("/", get(show_messages_handler))
        .route("/add", post(add_message_handler))
        .layer(Extension(app_state));

    let addr = "127.0.0.1:3000".parse().unwrap();

    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

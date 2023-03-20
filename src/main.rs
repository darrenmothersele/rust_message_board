use axum::{routing::get, Form, routing::post, Extension, Router, Server,
           extract::Query, };
use sqlx::{Sqlite, sqlite::SqlitePool};
use std::sync::{Arc};
use axum::response::{Html, IntoResponse, Redirect};
use sqlx::migrate::MigrateDatabase;
use serde::Deserialize;
use sanitize_html::sanitize_str;
use sanitize_html::rules::predefined::DEFAULT;

#[derive(Debug)]
struct AppState {
    db_pool: SqlitePool,
}

#[derive(sqlx::FromRow)]
struct Message {
    name: String,
    content: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
struct Pagination {
    offset: Option<u32>,
}

const PAGE_SIZE: u32 = 100;

#[axum_macros::debug_handler]
async fn show_messages_handler(state: Extension<Arc<AppState>>, pagination: Query<Pagination>) -> impl IntoResponse {
    let mut conn = state.db_pool.acquire().await.unwrap();
    let messages = sqlx::query_as::<_, Message>("SELECT * FROM messages ORDER BY created_at DESC LIMIT ? OFFSET ?")
        .bind(PAGE_SIZE)
        .bind(pagination.offset.unwrap_or(0))
        .fetch_all(&mut conn)
        .await
        .unwrap();

    let message_list = messages.iter().enumerate().fold(String::new(), |acc, (_i, msg)| {
        acc + &format!("<div><h3>{} <span>{}</span></h3><p>{}</p></div>\n", msg.name, msg.created_at, msg.content)
    });

    let start: String = if pagination.offset.unwrap_or(0) > 0 {
        r#"<a href="/">&laquo; back</a>"#.to_string()
    } else {
        "".to_string()
    };
    let next: String = if messages.len() >= PAGE_SIZE as usize {
        format!(r#"<a href="/?offset={}">more &raquo;</a>"#,
                pagination.offset.unwrap_or(0) + PAGE_SIZE)
    } else {
        "".to_string()
    };

    Html(format!(
        r#"
            <html>
            <head>
                <title>Message Board</title>
                <style>
                h1 {{ font-size: 1.5rem; }}
                body {{ font-family: sans-serif; }}
                input {{ display: block; width: 100%; margin-bottom: 0.5rem; }}
                textarea {{ display: block; width: 100%; margin-bottom: 0.5rem; }}
                label {{ display: block; width: 100%; }}
                form > h2 {{ font-size: 1rem; font-weight: bold; text-transform: uppercase; padding: 0.25rem 0.5rem;
                background: #eee; margin: 0; }}
                form {{ border: 1px solid #eee; margin-bottom: 1rem; }}
                .form-wrapper {{ padding: 1rem; }}
                .messages > h2 {{ font-size: 1rem; font-weight: bold; text-transform: uppercase; padding: 0.25rem 0.5rem;
                background: #eee; margin: 2rem 0 1rem 0; }}
                .messages > div {{ padding: 1rem 1rem 0 1rem; border: 1px solid #eee; margin-bottom: 1rem; }}
                .messages h3 {{ font-weight: normal; font-size: 1.125rem; margin: 0; }}
                .messages h3 > span {{ font-size: 0.75rem; }}
                .pagination {{ display: flex; justify-content: space-between; }}
                </style>
            </head>
            <body>
            <h1>Message Board</h1>

            <form method="POST" action="/add">
            <h2>Add message</h2>
            <div class="form-wrapper">
            <label for="name-input">Name:</label>
            <input id="name-input" type="text" name="name" />
            <label for="message-input">Message:</label>
            <textarea rows="5" id="message-input" name="message"></textarea>
            <button type="submit">Save</button>
            </div>
            </form>

            <div class="messages">
            <h2>Messages:</h2>
            {}
            </div>

            <div class="pagination">
            <span>{}</span>
            <span>{}</span>
            </div>

            </body>
            </html>
        "#,
        message_list,
        start,
        next
    ))
}


#[derive(Deserialize)]
struct MessageForm {
    name: String,
    message: String,
}

#[axum_macros::debug_handler]
async fn add_message_handler(Extension(state): Extension<Arc<AppState>>, Form(message_form): Form<MessageForm>) -> impl IntoResponse {
    let mut conn = state.db_pool.acquire().await.unwrap();
    sqlx::query("INSERT INTO messages (name, content) VALUES (?, ?)")
        .bind(sanitize_str(&DEFAULT, &message_form.name).unwrap_or_else(|_| String::from("")))
        .bind(sanitize_str(&DEFAULT, &message_form.message).unwrap_or_else(|_| String::from("")))
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

    let addr = "0.0.0.0:3000".parse().unwrap();

    println!("Launching server on http://127.0.0.1:3000");
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

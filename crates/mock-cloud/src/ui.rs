use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        // Example UI endpoints
        .route("/mock/ui", get(ui_index))
}

async fn ui_index() -> impl IntoResponse {
    Html(r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Mock Cloud Simulator UI</title>
            <style>
                body { font-family: sans-serif; padding: 2rem; background: #f4f4f5; }
                .container { max-width: 800px; margin: 0 auto; background: white; padding: 2rem; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.1); }
                h1 { color: #3b82f6; }
                a { color: #2563eb; text-decoration: none; font-weight: bold; }
            </style>
        </head>
        <body>
            <div class="container">
                <h1>Pollen Mock Cloud UI</h1>
                <p>Welcome to the Pollen Mock Cloud testing simulator.</p>
                <ul>
                    <li><a href="https://127.0.0.1:43892/admin/dashboard">Telemetry Dashboard</a></li>
                    <li><a href="/admin/registry">Registry State</a></li>
                </ul>
            </div>
        </body>
        </html>
    "#)
}

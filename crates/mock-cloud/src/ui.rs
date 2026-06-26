// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 AEC Infraconnect

use crate::state::AppState;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};

pub fn router() -> Router<AppState> {
    Router::new()
        // Example UI endpoints
        .route("/mock/ui", get(ui_index))
}

async fn ui_index() -> impl IntoResponse {
    Html(
        r#"
        <!DOCTYPE html>
        <html>
        <head>
            <title>Mock Cloud Simulator UI</title>
            <style>
                body { font-family: 'Inter', sans-serif; padding: 2rem; background: #0f172a; color: #f8fafc; }
                .container { max-width: 1000px; margin: 0 auto; background: #1e293b; padding: 2rem; border-radius: 8px; box-shadow: 0 4px 6px rgba(0,0,0,0.3); border: 1px solid #334155; }
                h1 { color: #3b82f6; border-bottom: 1px solid #334155; padding-bottom: 1rem; }
                .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); gap: 1rem; margin-top: 1.5rem; }
                .card { background: #0f172a; padding: 1.5rem; border-radius: 8px; border: 1px solid #334155; transition: transform 0.2s; }
                .card:hover { transform: translateY(-2px); border-color: #3b82f6; }
                .card h2 { margin-top: 0; font-size: 1.25rem; color: #e2e8f0; }
                .card p { color: #94a3b8; font-size: 0.875rem; line-height: 1.4; }
                a { color: #3b82f6; text-decoration: none; font-weight: 600; display: inline-block; margin-top: 1rem; }
                a:hover { text-decoration: underline; }
            </style>
        </head>
        <body>
            <div class="container">
                <h1>Pollek Mock Cloud Simulator</h1>
                <p style="color: #94a3b8;">Welcome to the local cloud simulator for Pollek DEK. Select a module below to inspect state.</p>
                
                <div class="grid">
                    <div class="card">
                        <h2>Devices & Telemetry</h2>
                        <p>View enrolled DEK devices, current health status, and live decision/telemetry logs.</p>
                        <a href="/admin/dashboard">Open Dashboard &rarr;</a>
                    </div>
                    
                    <div class="card">
                        <h2>Registry State</h2>
                        <p>Inspect tenants, principals, AI agents, MCP servers, and resources.</p>
                        <a href="/admin/registry">View Registry &rarr;</a>
                    </div>

                    <div class="card">
                        <h2>Policies & Bundles</h2>
                        <p>Browse active policies, PEP deployments, and TUF bundle generations.</p>
                        <a href="/admin/registry">View Policies &rarr;</a>
                    </div>

                    <div class="card">
                        <h2>Security Events</h2>
                        <p>Monitor simulated threat events, chaos outages, and bundle poisoning events.</p>
                        <a href="/mock/admin/audits">View Audits &rarr;</a>
                    </div>

                    <div class="card">
                        <h2>Runtime Metrics</h2>
                        <p>Prometheus metrics exported by the mock cloud and DEK agents.</p>
                        <a href="/metrics" target="_blank">View Metrics &rarr;</a>
                    </div>

                    <div class="card">
                        <h2>Update Server (TUF)</h2>
                        <p>Inspect the simulated TUF repository and update metadata.</p>
                        <a href="/v1/update/1/metadata/timestamp.json" target="_blank">View Timestamp &rarr;</a>
                    </div>
                </div>
            </div>
        </body>
        </html>
    "#,
    )
}

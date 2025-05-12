use actix_web::{web, HttpResponse, Responder};
use tera::Context;
use uuid::Uuid;

use crate::{AppState, render};

pub async fn online_users(app_state: web::Data<AppState>) -> impl Responder {
    let manager = app_state.connection_manager.lock().await;

    let mut ctx = Context::new();

    let users: Vec<_> = manager.user_data.iter().map(|(uuid, data)| {
        let connected = manager.user_connections.contains_key(uuid);
        serde_json::json!({
            "uuid": uuid.to_string(),
            "email": data.email,
            "pseudo": data.pseudo,
            "connected": !connected,
        })
    }).collect();

    //log::info!
    for user in &users {
    log::info!(
        "UUID: {}, Email: {}, Pseudo: {}, WebSocket: {}",
        user["uuid"],
        user["email"],
        user["pseudo"],
        if user["connected"].as_bool().unwrap_or(false) { "✅" } else { "❌" }
    );
}

    ctx.insert("users", &users);

    render("admin/online_users.html", ctx)
}

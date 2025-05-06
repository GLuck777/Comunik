mod handlers;           // Ajoute le fichier handlers.rs
use handlers::*;        // Cela permet d'utiliser les fonctions définies dans handlers.rs
mod sql_req;
use std::env;
use env_logger;
mod ws_init;



// use actix_files::Files;
use actix_web::{web, App, HttpServer, HttpRequest, HttpResponse, middleware};
use actix_web_actors::ws as other_ws;
mod ws;
use actix_session::{Session}; // supprime CookieSession
use tera::Tera;  // Pour les templates
// use std::sync::Arc;
// use uuid::Uuid;  // Pour générer des UUID
use sqlx::SqlitePool;
// use actix_web::{App, HttpServer, middleware};
use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::cookie::Key;
use actix_files::Files; // Pour ajouter le CSS dans les pages

// New imports
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use uuid::Uuid;
use tokio::sync::broadcast;
use crate::ws::ServerMessage;

#[derive(Debug, Clone)]
pub struct AppState {
    db_pool: SqlitePool,
    rooms: Arc<Mutex<HashMap<Uuid, broadcast::Sender<ServerMessage>>>>,
}

// Perform WebSocket handshake and start actor.
// #[get("/ws/{room_uuid}")]
pub async fn websocket_handler(
    req: HttpRequest, 
    stream: web::Payload, 
    session: Session,
    app_state: web::Data<AppState>,
    // pool: web::Data<SqlitePool>,
    path: Option<web::Path<String>> // <- ici
) -> Result<HttpResponse, actix_web::Error> { //ajout: pool: web::Data<SqlitePool>
    // other_ws::start(ws::MyWebSocket { db_pool: pool.get_ref().clone() }, &req, stream)
    let room_uuid = path.map(|p| p.into_inner());
    if let Some(uuid) = session.get::<String>("uuid")? {
        let ws = ws::MyWebSocket {
            db_pool: app_state.db_pool.clone(),
            user_uuid: uuid,
            room_uuid,
            rooms: Arc::clone(&app_state.rooms),
        };
        println!("\x1b[0;31m Start! \x1b[0m");
        other_ws::start(ws, &req, stream)
    } else {
        println!("\x1b[0;31m cinquieme partie de test \x1b[0m");
        Err(actix_web::error::ErrorUnauthorized("No UUID in session"))
    }
    // other_ws::start(ws::MyWebSocket {}, &req, stream) // ancienne version sans pool
}


use once_cell::sync::Lazy;

static TEMPLATES: Lazy<Tera> = Lazy::new(|| {
    match Tera::new("templates/**/*") {
        Ok(t) => t,
        Err(e) => {
            println!("Parsing error(s): {}", e);
            std::process::exit(1);
        }
    }
});

fn render(template_name: &str, context: tera::Context) -> HttpResponse {
    let rendered = TEMPLATES.render(template_name, &context);
    match rendered {
        Ok(html) => HttpResponse::Ok().content_type("text/html").body(html),
        Err(e) => HttpResponse::InternalServerError().body(format!("Template error: {}", e)),
    }
}

#[actix_web::main]

async fn main() -> std::io::Result<()> {
    std::env::set_var("RUST_LOG", "sqlx=debug");
    env_logger::init();
    //let secret_key = Key::generate(); // En prod, utilise une clé fixe et sûre !
    //let secret_key = Key::from(&[0; 64]); // 64 bytes (mauvais en prod, mais ok pour tests)
    let secret_key = Key::from(b"0123456789012345678901234567890123456789012345678901234567890123");


    println!("Serveur démarré sur http://localhost:8080");
    let pool = SqlitePool::connect("sqlite:./app.db").await.unwrap();

    let rooms = Arc::new(Mutex::new(HashMap::new()));
    
    // Initialize room broadcast channels from database
    if let Err(e) = ws_init::initialize_rooms(&pool, &rooms).await {
        eprintln!("Failed to initialize rooms: {}", e);
    }
    let app_state = AppState {
        db_pool: pool.clone(),
        rooms: rooms.clone(),
    };

    HttpServer::new(move|| {
        
        App::new()
            .app_data(web::Data::new(app_state.clone()))
            .app_data(web::Data::new(pool.clone()))
            .wrap(SessionMiddleware::builder(
                CookieSessionStore::default(),
                secret_key.clone(),
            )
            .cookie_secure(false) // pour local, true en prod
            .cookie_name("comunik_session".to_string()) // optionnel
            .build())
            .service(Files::new("/static", "./static").show_files_listing()) // enlever show_files_listening une fois le developpement effectué
            .service(web::resource("/access").route(web::get().to(handlers::access_form)).route(web::post().to(handlers::access_check)))
            .service(web::resource("/login").route(web::get().to(handlers::login_form)).route(web::post().to(handlers::login_post)))
            .service(web::resource("/register").route(web::get().to(handlers::register_form)).route(web::post().to(handlers::register_post)))
            .service(web::resource("/logout").route(web::post().to(handlers::logout)))
            .service(web::resource("/home").route(web::get().to(handlers::home)))
            .service(web::resource("/profile").route(web::get().to(handlers::profile)))
            .service(web::resource("/profile/edit").route(web::post().to(handlers::update_profile)))
            .service(web::resource("/api/rooms/{user_uuid}").route(web::get().to(get_room_by_uuid)))
            .service(web::resource("/api/notif/{user_uuid}").route(web::get().to(get_user_notifications)))
            .service(web::resource("/api/notifications/{user_uuid}").route(web::get().to(get_notifications)))
            .service(web::resource("/api/notificationsD/{id}").route(web::delete().to(delete_notification)))
            .service(web::resource("/ws/").route(web::get().to(websocket_handler)))
            .service(web::resource("/ws/{room_uuid}").route(web::get().to(websocket_handler)))
            .route("/", web::get().to(|| async {
                actix_web::HttpResponse::Found()
                    .append_header(("Location", "/access"))
                    .finish()
            }))
            // .route("/ws/", web::get().to(websocket_handler)) //bonne route fonctionnel mais canard noir
            // .service(Files::new("/", "./static").index_file("access.html")) // <- commenté
    })    
    .bind(("0.0.0.0", 8080))?
    .run()
    .await
}


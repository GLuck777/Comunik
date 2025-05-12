// use tools::*;
use crate::{sql_req::*, AppState};

use actix_web::{web, HttpResponse, Responder};
use actix_session::Session;
use serde::{Serialize, Deserialize};
use tera::Context;
use uuid::Uuid;
use sqlx::{SqlitePool, Row, FromRow};
use rand::{Rng, distributions::Alphanumeric};
use argon2::{Argon2, PasswordHasher, PasswordVerifier, PasswordHash as PH};
use password_hash::{SaltString, PasswordHash, rand_core::OsRng};

// new 
use crate::connection_manager::UserData;

// use std::io::{self, Read};
use crate::render;

// use serde::{Serialize, Deserialize};
use chrono::{Utc, Duration};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,         // UUID de l'utilisateur
    email: String,
    pseudo: String,
    exp: usize,          // Expiration du token (timestamp)
}

use jsonwebtoken::{decode, DecodingKey, Validation};
use actix_web::HttpRequest;

//lecture du token dans les handlers
fn extract_claims_from_token(req: &HttpRequest) -> Option<Claims> {
    let jwt_secret = "comunik_session_key";
    let token_cookie = req.cookie("auth_token")?;
    let token_data = decode::<Claims>(
        token_cookie.value(),
        &DecodingKey::from_secret(jwt_secret.as_ref()),
        &Validation::default()
    ).ok()?;
    Some(token_data.claims)
}

#[derive(Deserialize)]
pub struct AccessForm {
    code: String,
}

#[derive(Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginForm {
    // pub username: String,
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct Notification {
    pub id: i64,
    pub user_uuid: String,
    pub message: String,
}


#[derive(Serialize,FromRow, Debug)]
pub struct Room {
    pub id: i64,
    pub name: String,
    pub room_uuid: String,
    pub member_count: i64,  // Nombre de membres dans la salle
}

#[derive(Serialize,FromRow, Debug)]
struct Rooms {
    id: i64,
    room_uuid: String,
    name: String,
    owner_uuid: String,
    member_count: i64,  // Nombre de membres dans la salle
}

//if let Ok(Some(_)) = session.get::<String>("uuid") { // restriction sur la possibilité de l'utilisateur 

pub async fn access_form(session: Session) -> impl Responder {
    if let Ok(Some(_)) = session.get::<String>("uuid") {
        return HttpResponse::Found().append_header(("Location", "/home")).finish();
    }
    let ctx = tera::Context::new();
    render("access.html", ctx)
}

pub async fn access_check(session: Session, form: web::Form<AccessForm>) -> impl Responder {
    if form.code == "ACCESS01" {
        session.insert("access_granted", true).unwrap();
        HttpResponse::Found().append_header(("Location", "/login")).finish()
    } else {
        let mut ctx = tera::Context::new();
        ctx.insert("error", "Code incorrect");
        render("access.html", ctx)
    }
}

async fn generate_unique_pseudo(pool: &SqlitePool) -> String {
    loop {
        let pseudo: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(15)
            .map(char::from)
            .collect();

        // Vérifie si le pseudo existe déjà
        let exists: Option<(String,)> = sqlx::query_as("SELECT pseudo FROM users WHERE pseudo = ?")
            .bind(&pseudo)
            .fetch_optional(pool)
            .await
            .unwrap();

        if exists.is_none() {
            return pseudo;
        }
    }
}

// use actix_web::{web, HttpResponse, Responder};

pub async fn login_form(session: Session) -> impl Responder {
    if let Ok(Some(_)) = session.get::<String>("uuid") {
        return HttpResponse::Found()
            .append_header(("Location", "/home"))
            .finish();
    }

    let ctx = tera::Context::new();
    render("login.html", ctx)
}

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::ConnectionManager;
pub async fn login_post(
    form: web::Form<LoginForm>,
    pool: web::Data<SqlitePool>,
    session: Session,
    // connection_manager: web::Data<Arc<Mutex<ConnectionManager>>>, // Nouveau paramètre
    app_state: web::Data<AppState>
) -> impl Responder {
    let row = sqlx::query(
        "SELECT uuid, pseudo, password FROM users WHERE email = ? LIMIT 1"
        )
        .bind(&form.email) // Bind de l'email
        .fetch_optional(pool.get_ref()) // Récupère la ligne correspondante
        .await;

        match row {
            Ok(Some(row)) => {
                // Extraire le UUID et le mot de passe hashé de la ligne
                let uuid: String = row.get("uuid");
                let hashed_password: String = row.get("password");
                let pseudo: String = row.get("pseudo");
                let email: String = form.email.clone();
                // form.username = pseudo;
    
                // Vérifier le mot de passe
                let parsed_hash = PasswordHash::new(&hashed_password);
                match parsed_hash {
                    Ok(ph) => {
                        if Argon2::default()
                            .verify_password(form.password.as_bytes(), &ph)
                            .is_ok()
                        {
                            // Authentification réussie, on stocke l'UUID en session
                            // session.insert("uuid", uuid.clone()).unwrap_or(());
                            // session.insert("email", form.email.clone()).unwrap_or(());
    
                            println!("Email de l'utilisateur: {}", form.email); // form.username
                            // session.insert("pseudo", &pseudo).unwrap_or(());
                            session.insert("uuid", uuid.clone()).unwrap();
                            // creation du token json
                            use jsonwebtoken::{encode, Header, EncodingKey};

                            let expiration = Utc::now()
                                .checked_add_signed(Duration::hours(24))
                                .unwrap()
                                .timestamp() as usize;

                            let claims = Claims {
                                sub: uuid.clone(),
                                email: form.email.clone(),
                                pseudo: pseudo.clone(),
                                exp: expiration,
                            };

                            let jwt_secret = "comunik_session_key"; // stocke-le dans une variable d'environnement en prod !
                            let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(jwt_secret.as_ref())).unwrap();
                            // fin de creation du token json
                            session.insert("email", form.email.clone()).unwrap();
                            session.insert("pseudo", &pseudo).unwrap();

                            // Créer le contexte avec les données utilisateur
                            let mut ctx = Context::new();
                            ctx.insert("uuid", &uuid);
                            ctx.insert("email", &email);
                            ctx.insert("username", &pseudo); // ou "pseudo"

                            println!("Utilisateur connecté : {} ({}) - uuid: {}", pseudo, email, uuid);
                            //new instance for register data
                            // Enregistrer les données de l'utilisateur dans le ConnectionManager
                            // Même si le websocket n'est pas encore connecté, ces données seront disponibles
                            // info apprentissage sur les {...}:

                            // bloc de portée (scope), et c'est une protection explicite.Et ce verrou (manager) reste actif tant qu’il est en vie. Si tu ne le libères pas, le verrou est conservé jusqu'à la fin de la fonction — ce qui peut bloquer d’autres tâches qui veulent aussi accéder à ce Mutex.
                            // En plaçant cette opération dans un bloc { ... } :
                            // Tu restreins la durée de vie du verrou, donc :
                            //  manager est drop() dès que le bloc se termine.
                            // Le Mutex est libéré plus tôt, permettant à d'autres .lock().await d’accéder à connection_manager.
                            {
                                let mut manager = app_state.connection_manager.lock().await;
                                let user_data = UserData {
                                    email: form.email.clone(),
                                    pseudo: pseudo.clone(),
                                    last_activity: chrono::Utc::now(),
                                };
                                
                                // Nous ne pouvons pas encore enregistrer de connection (websocket), 
                                // mais nous pouvons stocker les données
                                manager.user_data.insert(uuid.clone(), user_data).unwrap();
                                println!("=== Utilisateurs connectés ===");
                                for (uuid, data) in manager.user_data.iter() {
                                    println!("- UUID: {}", uuid);
                                    println!("  Email: {}", data.email);
                                    println!("  Pseudo: {}", data.pseudo);
                                    if let Some(addr) = manager.user_connections.get(uuid) {
                                        println!("  WebSocket actif: ✅");
                                    } else {
                                        println!("  WebSocket actif: ❌");
                                    }
                                }
                                println!("==============================");
                            }

                            // return HttpResponse::Found()
                            // .append_header(("Location", "/home"))
                            // .finish();
                            return HttpResponse::Found()
                            .append_header(("Location", "/home"))
                            .cookie(
                                actix_web::cookie::Cookie::build("auth_token", token)
                                    .path("/")
                                    .http_only(true) // sécurité
                                    .finish()
                            )
                            .finish();
                        }
                    }
                    Err(_) => {}
                }
    
                // Mot de passe incorrect
                let mut ctx = Context::new();
                ctx.insert("error", "Mot de passe incorrect.");
                render("login.html", ctx)
            }
            Ok(None) => {
                // Si aucun utilisateur n'a été trouvé avec cet email
                let mut ctx = Context::new();
                ctx.insert("error", "Email introuvable.");
                render("login.html", ctx)
            }
            Err(_) => {
                // Si une erreur survient lors de la requête SQL
                HttpResponse::InternalServerError().body("Erreur serveur")
            }
        }
    }



pub async fn register_form(session: Session) -> impl Responder {
    if let Ok(Some(_)) = session.get::<String>("uuid") {
        return HttpResponse::Found()
            .append_header(("Location", "/home"))
            .finish();
    }

    let ctx = tera::Context::new();
    render("register.html", ctx)
}

pub async fn register_post(
    form: web::Form<RegisterForm>,
    pool: web::Data<SqlitePool>,
) -> impl Responder {
    let uuid = Uuid::new_v4().to_string();

    // Hasher le mot de passe
    // Générer un salt sécurisé aléatoire
    let salt = SaltString::generate(&mut OsRng);

    // Hasher le mot de passe
    let argon2 = Argon2::default();
    let hashed_password = argon2.hash_password(form.password.as_bytes(), &salt)
        .expect("Erreur lors du hash du mot de passe")
        .to_string();

    // `password_hash` est une string que tu peux stocker dans la base de données

    // Générer un pseudo unique
    let pseudo = generate_unique_pseudo(&pool).await;

    // Insérer dans la base
    let result = sqlx::query(
        "INSERT INTO users (uuid, email, password, pseudo) VALUES (?, ?, ?, ?)",
    )
    .bind(uuid)
    .bind(&form.email)
    .bind(&hashed_password)
    .bind(&pseudo)
    .execute(pool.get_ref())
    .await;

    match result {
        Ok(_) => {
            // après l'inscription réussit connexion de l'individu et redirection vers /home
                let mut ctx = Context::new();

                ctx.insert("email", &form.email.clone());
                ctx.insert("username", &pseudo); // ou "pseudo"

                println!("Utilisateur connecté : {} ({})", pseudo, &form.email.clone());

                // return render("home.html", ctx);
                return HttpResponse::Found()
                            .append_header(("Location", "/home"))
                            .finish();
        },
        Err(e) => HttpResponse::InternalServerError().body(format!("Erreur DB: {}", e)),
    }
}

// permet de supprimer le coockie généré
use actix_web::cookie::{Cookie, time::OffsetDateTime};

pub async fn logout(session: Session) -> impl Responder {
    session.purge(); // ou session.clear();

    // Crée un cookie "auth_token" expiré
    let expired_cookie = Cookie::build("auth_token", "")
        .expires(OffsetDateTime::UNIX_EPOCH) // Expiré depuis 1970 pour bien montrer qu'il est moisi
        .finish();

    HttpResponse::Found()
        .append_header(("Location", "/login"))
        .cookie(expired_cookie) // Supprime côté client
        .finish()
    // HttpResponse::Found().append_header(("Location", "/login")).finish()
}

pub async fn home(req: HttpRequest, session: Session, pool: web::Data<SqlitePool>) -> impl Responder {
    if let Ok(Some(uuid)) = session.get::<String>("uuid") {
        let email = session.get::<String>("email").unwrap_or(None).unwrap_or_default();
        let usernamebis = session.get::<String>("pseudo").unwrap_or(None).unwrap_or_default();
        let username = match get_user_name(&pool, &uuid).await{
            Ok(name) => name,
            Err(_) => usernamebis,
        };

        // Récupérer les rooms associées à cet utilisateur
        let rooms = get_rooms_for_user(&pool, &uuid).await.unwrap_or_else(|_| Vec::new());

        // Vérifier le contenu de rooms avant d'envoyer aux templates
        println!("\n\tRooms: {:?}", rooms); // Affiche les rooms dans le terminal


        if let Some(claims) = extract_claims_from_token(&req) {
            let mut ctx = Context::new();
            ctx.insert("uuid", &claims.sub);
            ctx.insert("email", &claims.email);
            ctx.insert("username", &claims.pseudo);
            ctx.insert("rooms", &rooms);
            render("home.html", ctx)
        } else {
            HttpResponse::Found().append_header(("Location", "/login")).finish() // doublon...
        }
        // Insérer dans le contexte Tera
        // let mut ctx = Context::new();
        // ctx.insert("email", &email);
        // ctx.insert("username", &username);
        // ctx.insert("uuid", &uuid);
        // ctx.insert("rooms", &rooms);
        // render("home.html", ctx)
    } else {
        return HttpResponse::Found().append_header(("Location", "/login")).finish();
    }
}

async fn get_rooms_for_user(pool: &web::Data<SqlitePool>, user_uuid: &str) -> Result<Vec<Rooms>, sqlx::Error> {
    println!("\n\tj'entre dans get_rooms_for_user\n");
    
    let rooms = sqlx::query_as::<_, Rooms>(
        "SELECT r.id, r.room_uuid, r.name, r.owner_uuid, r.created_at, 
        (SELECT COUNT(*) FROM room_members rm WHERE rm.room_uuid = r.room_uuid) AS member_count
         FROM users u
         INNER JOIN room_members rm ON u.uuid = rm.user_uuid
         INNER JOIN rooms r ON rm.room_uuid = r.room_uuid
         WHERE u.uuid = ?"
    )
    .bind(user_uuid)
    .fetch_all(pool.get_ref())
    .await;
    // ajouter tu as la room, combien de room_member par rooms pour l'affichage en réel home.html

    match rooms {
        Ok(data) => Ok(data),  // Si la requête réussit, retourne les rooms
        Err(e) => {
            println!("Erreur lors de la sélection dans rooms pour home: {:?}", e);
            Err(e)  // Si la requête échoue, retourne l'erreur
        }
    }
}
pub async fn get_room_by_uuid(
    pool: web::Data<SqlitePool>,
    room_uuid: web::Path<String>,
) -> Result<HttpResponse, actix_web::Error> {
    let room_uuid = room_uuid.into_inner();
    println!("nhbfebbebnfjerfb ==>Q {}", room_uuid);

    let result = sqlx::query_as::<_, Room>(
        r#"
        SELECT 
            r.id,
            r.name,
            r.room_uuid,
            (SELECT COUNT(*) FROM room_members rm WHERE rm.room_uuid = r.room_uuid) AS member_count
        FROM rooms r
        WHERE r.room_uuid = ?
        LIMIT 1
        "#
    )
    .bind(&room_uuid)
    .fetch_one(pool.get_ref())
    .await;

    match result {
        Ok(room) => Ok(HttpResponse::Ok().json(room)),
        Err(e) => {
            eprintln!("Erreur lors de la récupération de la room {}: {:?}", room_uuid, e);
            Ok(HttpResponse::NotFound().body("Room non trouvée"))
        }
    }
}

// SQL basique qui sert de modele classique voir sql_res.rs pour voir la fonction d'insertion de message
async fn save_message(pool: &SqlitePool, room_uuid: &str, user_uuid: &str, content: &str) {
    let result = sqlx::query(
        "INSERT INTO messages (room_uuid, user_uuid, content) VALUES (?, ?, ?)")
        .bind(room_uuid)
        .bind(user_uuid)
        .bind(content)
        .execute(pool)
        .await
        .expect("Failed to insert message into database");
}
// Fonction pour récupérer les notifications depuis la base de données
// GET /api/notif/
pub async fn get_user_notifications(
    pool: web::Data<SqlitePool>,
    user_uuid: web::Path<String>,
) -> Result<HttpResponse, actix_web::Error> {
    println!("IXION get user notification");
    let user_uuid = user_uuid.into_inner();
    let result = sqlx::query_as::<_, Notification>(
        "SELECT id, user_uuid, message FROM notifications WHERE user_uuid = ?"
    )
    .bind(&user_uuid)
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(notifs) => Ok(HttpResponse::Ok().json(notifs)),  // Retourner un HttpResponse avec les notifications
        Err(e) => {
            eprintln!("Erreur SQL: {:?}", e);
            Ok(HttpResponse::InternalServerError().finish())  // Retourner une erreur 500 si la requête échoue
        }
    }
}
// GET /api/notifications/:user_uuid
pub async fn get_notifications(
    pool: web::Data<SqlitePool>,
    user_uuid: web::Path<String>,
) -> Result<HttpResponse, actix_web::Error> {
    let user_uuid = user_uuid.into_inner();

    let result = sqlx::query_as::<_, Notification>(
        "SELECT id, user_uuid, message FROM notifications WHERE user_uuid = ?"
    )
    .bind(&user_uuid)
    .fetch_all(pool.get_ref())
    .await;

    match result {
        Ok(notifs) => Ok(HttpResponse::Ok().json(notifs)),
        Err(e) => {
            eprintln!("Erreur SQL: {:?}", e);
            Ok(HttpResponse::InternalServerError().finish())
        }
    }
}

// DELETE /api/notifications/:id
pub async fn delete_notification(
    pool: web::Data<SqlitePool>,
    path: web::Path<i64>,
) -> impl Responder {
    let id = path.into_inner();
    println!("delete {}", id);

    let result = sqlx::query("DELETE FROM notifications WHERE id = ?")
        .bind(id)
        .execute(pool.get_ref())
        .await;

        match result {
            Ok(_) => HttpResponse::NoContent().finish(),
            Err(e) => {
                eprintln!("Erreur de suppression: {:?}", e); // Log de l'erreur
                HttpResponse::InternalServerError().finish()
            }
        }
}




pub async fn profile(session: Session, pool: web::Data<SqlitePool>) -> impl Responder {
    println!("Tentative d'accès à /profile");
    // Vérifier si l'utilisateur est authentifié (uuid stocké en session)
    if let Ok(Some(uuid)) = session.get::<String>("uuid") {
        println!("UUID trouvé en session : {}", uuid);
        // Effectuer la requête pour récupérer l'email et le pseudo
        let row = sqlx::query(
            "SELECT email, pseudo FROM users WHERE uuid = ?"
        )
        .bind(&uuid) // Bind de l'UUID
        .fetch_optional(pool.get_ref()) // Récupère la ligne correspondante
        .await;

        match row {
            Ok(Some(row)) => {
                // Si la ligne est trouvée, on récupère l'email et le pseudo
                let email: String = row.get("email");
                let pseudo: String = row.get("pseudo");

                // Préparer le contexte pour le rendu de la page
                let mut ctx = tera::Context::new();
                ctx.insert("email", &email);
                ctx.insert("pseudo", &pseudo);

                // Rendre la page profile.html avec les informations de l'utilisateur
                return crate::render("profile.html", ctx);
            }
            Ok(None) => {
                // Si aucun utilisateur n'a été trouvé avec cet UUID (cas improbable)
                return HttpResponse::InternalServerError().body("Utilisateur introuvable");
            }
            Err(_) => {
                // En cas d'erreur dans la requête SQL
                return HttpResponse::InternalServerError().body("Erreur lors de la récupération des données utilisateur");
            }
        }
    } else {
        println!("Aucun UUID en session, redirection vers /login");
    }

    // Si l'utilisateur n'est pas connecté (aucun UUID en session), redirection vers la page de connexion
    HttpResponse::Found()
        .append_header(("Location", "/login"))
        .finish()
}

#[derive(serde::Deserialize)]
pub struct ProfileUpdateData {
    email: String,
    pseudo: String,
}

pub async fn update_profile(
    session: Session,
    pool: web::Data<SqlitePool>,
    form: web::Json<ProfileUpdateData>,
) -> impl Responder {
    println!("Tentative de update du profile......");
    if let Ok(Some(uuid)) = session.get::<String>("uuid") {
        let res = sqlx::query("UPDATE users SET email = ?, pseudo = ? WHERE uuid = ?")
            .bind(&form.email)
            .bind(&form.pseudo)
            .bind(&uuid)
            .execute(pool.get_ref())
            .await;

        match res {
            Ok(_) => HttpResponse::Ok().body("Profil mis à jour !"),
            Err(e) => {
                println!("Erreur SQL : {:?}", e);
                HttpResponse::InternalServerError().body("Erreur lors de la mise à jour.")
            }
        }
    } else {
        HttpResponse::Unauthorized().body("Non connecté")
    }
}
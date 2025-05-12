use crate::connection_manager::ConnectionManager;
use actix_session::Session;
use actix::{Actor, StreamHandler, Addr};  // Ajout d'Addr pour gérer les références d'acteurs
use actix::AsyncContext;
use actix::fut;
use actix::ActorFutureExt;
use crate::other_ws::WebsocketContext;

use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use sqlx::{Any, SqlitePool, Row};
use crate::sql_req::*;
use tokio::spawn;
use serde_json::Value;

use serde_json::json;
use tokio::sync::broadcast;
use std::collections::HashMap;
use uuid::Uuid;
use std::sync::Arc;
use std::sync::Mutex;

use actix::Message;
use actix::Handler;

// Structure de message pour les communications directes entre acteurs
#[derive(Message, Clone)]
#[rtype(result = "()")]
pub struct DirectMessage {
    pub content: String,
    pub message_type: String,
    pub from_user: String,
    pub room_uuid: Option<String>,
    pub extra_data: Option<Value>,
}

// Implémentation du handler pour DirectMessage
impl Handler<DirectMessage> for MyWebSocket {
    type Result = ();

    fn handle(&mut self, msg: DirectMessage, ctx: &mut Self::Context) {
        let server_msg = ServerMessage {
            msg: msg.content.clone(),
            r#type: msg.message_type,
            message: msg.content,
            name: None,
            room_uuid: msg.room_uuid,
            user_uuid: Some(msg.from_user),
            json: msg.extra_data,
        };
        
        if let Ok(json) = serde_json::to_string(&server_msg) {
            ctx.text(json);
        }
    }
}

// Message simple (maintenu pour compatibilité)
#[derive(Message)]
#[rtype(result = "()")]
pub struct MyMessage(pub String);

impl Handler<MyMessage> for MyWebSocket {
    type Result = ();

    fn handle(&mut self, msg: MyMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "create_room")]
    CreateRoom { name: String, visibility: String, invitees: Vec<String>},

    #[serde(rename = "chat")]
    Chat { room_uuid: String, message: String },

    #[serde(rename = "join_room")]
    GetRoomData { room_uuid: String },

    #[serde(rename = "fetch_users")]
    GetUsersData {},

    #[serde(rename = "add_members")]
    AddRoomMembers {room_uuid: String, invitees: Vec<String>},
    
    // Nouveau message direct
    #[serde(rename = "direct_message")]
    DirectMessage { target_user: String, message: String },

    #[serde(rename = "unread_message")]
    GetUnreadMessages {},
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerMessage {
    pub msg: String,
    pub r#type: String,
    pub message: String,
    pub name: Option<String>,
    pub room_uuid: Option<String>,
    pub user_uuid: Option<String>,
    pub json: Option<Value>
}

// Structure globale pour maintenir les connexions actives
// Ce type devrait être accessible globalement dans votre application
// pub struct ConnectionManager {
//     // Mapping UUID d'utilisateur -> Addr de WebSocket
//     pub user_connections: HashMap<String, Addr<MyWebSocket>>,
//     // Mapping UUID de room -> canal broadcast
//     pub room_channels: HashMap<Uuid, broadcast::Sender<ServerMessage>>,
// }

// impl ConnectionManager {
//     pub fn new() -> Self {
//         Self {
//             user_connections: HashMap::new(),
//             room_channels: HashMap::new(),
//         }
//     }
    
//     pub fn register_connection(&mut self, user_uuid: String, addr: Addr<MyWebSocket>) {
//         self.user_connections.insert(user_uuid, addr);
//     }
    
//     pub fn unregister_connection(&mut self, user_uuid: &str) {
//         self.user_connections.remove(user_uuid);
//     }
    
//     pub fn send_direct_message(&self, target_user: &str, message: DirectMessage) -> bool {
//         if let Some(addr) = self.user_connections.get(target_user) {
//             addr.do_send(message);
//             true
//         } else {
//             false
//         }
//     }
// }

// Créez une instance globale de ConnectionManager
lazy_static::lazy_static! {
    static ref CONNECTION_MANAGER: Arc<Mutex<ConnectionManager>> = Arc::new(Mutex::new(ConnectionManager::new()));
}

#[derive(Debug)]
pub struct MyWebSocket {
    pub db_pool: SqlitePool,
    pub user_uuid: String,
    pub room_uuid: Option<String>,
    pub rooms: Arc<Mutex<HashMap<Uuid, broadcast::Sender<ServerMessage>>>>,
}

impl Actor for MyWebSocket {
    type Context = WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Enregistrer cette connexion dans le gestionnaire global
        let addr = ctx.address();
        let user_uuid = self.user_uuid.clone();
        
        {
            let mut manager = CONNECTION_MANAGER.lock().unwrap();
            // manager.register_connection(user_uuid, addr);
            // Récupérer les données utilisateur si elles existent déjà (depuis login)
            let user_data = manager.user_data.remove(&user_uuid);
            manager.register_connection(user_uuid, addr, user_data);
        }
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        // Retirer cette connexion du gestionnaire
        {
            let mut manager = CONNECTION_MANAGER.lock().unwrap();
            manager.unregister_connection(&self.user_uuid);
        }
    }
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        println!("websocket message received!");
        if let Ok(ws::Message::Text(text)) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::CreateRoom { name, visibility, invitees }) => {
                    println!("create_room, name: {}", name);
                    
                    // Clone necessary data
                    let pool = self.db_pool.clone();
                    let uuid = self.user_uuid.clone();
                    let name_clone = name.clone();
                    let visibility_clone = visibility.clone();
                    let invitees_clone = invitees.clone();
                    let rooms = Arc::clone(&self.rooms);
                    
                    // Create a future for room creation
                    let fut = async move {
                        let mut sender_opt = None;
                    
                        let result = create_room_in_db(&pool, &name_clone, &visibility_clone, &uuid, &invitees_clone).await;
                        
                        let server_msg = match result {
                            Ok(room_uuid) => {
                                match Uuid::parse_str(&room_uuid) {
                                    Ok(room_uuid_parsed) => {
                                        let (tx, _rx) = broadcast::channel(100);
                                        
                                        {
                                            let mut rooms_guard = rooms.lock().unwrap();
                                            rooms_guard.insert(room_uuid_parsed, tx.clone());
                                            
                                            // Aussi mettre à jour le gestionnaire global de connexions
                                            let mut manager = CONNECTION_MANAGER.lock().unwrap();
                                            manager.room_channels.insert(room_uuid_parsed, tx.clone());
                                        }
                    
                                        // Enregistrer le sender
                                        sender_opt = Some(tx.clone());
                    
                                        // Création du message
                                        let server_msg = ServerMessage {
                                            msg: format!("Room '{}' créée avec succès par {}", name_clone, uuid),
                                            r#type: "room_created".to_string(),
                                            message: format!("Room '{}' créée avec succès par {}", name_clone, uuid),
                                            name: Some(name_clone),
                                            room_uuid: Some(room_uuid.clone()),
                                            user_uuid: Some(uuid.clone()),
                                            json: None,
                                        };
                    
                                        // Notification aux invités via message direct
                                        let manager = CONNECTION_MANAGER.lock().unwrap();
                                        let user_uuids = get_room_user_uuids(&pool, &room_uuid).await.unwrap_or_default();
                                        
                                        for invitee_uuid in &invitees_clone {
                                            let dm = DirectMessage {
                                                content: format!("Vous avez été invité à rejoindre la room '{}'", name.clone()),
                                                message_type: "room_invitation".to_string(),
                                                from_user: uuid.clone(),
                                                room_uuid: Some(room_uuid.clone()),
                                                extra_data: None,
                                            };
                                            manager.send_direct_message(invitee_uuid, dm);
                                        }
                    
                                        server_msg
                                    }
                                    Err(_) => ServerMessage {
                                        msg: "Erreur de format UUID".to_string(),
                                        r#type: "error".to_string(),
                                        message: "UUID invalide retourné".to_string(),
                                        name: None,
                                        room_uuid: None,
                                        user_uuid: None,
                                        json: None,
                                    },
                                }
                            }
                            Err(e) => ServerMessage {
                                msg: "Erreur création room".to_string(),
                                r#type: "error".to_string(),
                                message: format!("Erreur BDD: {}", e),
                                name: None,
                                room_uuid: None,
                                user_uuid: None,
                                json: None,
                            },
                        };
                    
                        (server_msg, sender_opt)
                    };
                    
                    ctx.spawn(
                        fut::wrap_future(fut).map(move |(server_msg, sender_opt), _, ctx: &mut WebsocketContext<MyWebSocket>| {
                            if let Some(sender) = sender_opt {
                                let _ = sender.send(server_msg.clone());
                            }
                            let json = serde_json::to_string(&server_msg).unwrap_or_default();
                            ctx.text(json);
                        }),
                    );
                }
                
                Ok(ClientMessage::AddRoomMembers{ room_uuid, invitees }) => {
                    println!("\x1b[0;31m AddRoomMembers! \x1b[0m");
                    let pool = self.db_pool.clone();
                    let user_uuid = self.user_uuid.clone();
                    let room_uuid_clone = room_uuid.clone();
                    let invitees_clone = invitees.clone();
                    let rooms = Arc::clone(&self.rooms);
                    let rooms_clone = Arc::clone(&rooms);
                    
                    let fut = async move {
                        let result = add_members_in_room(&pool, &room_uuid_clone, invitees_clone.clone()).await;
                    
                        match result {
                            Ok(added_users) => {
                                println!("\x1b[0;33m[Membres ajoutés dans la room]:\x1b[0m");
                                for (uuid, pseudo) in &added_users {
                                    println!("\x1b[0;33m - {} ({})\x1b[0m", pseudo, uuid);
                                }
                    
                                // Message pour celui qui a ajouté les membres
                                let server_msg = ServerMessage {
                                    msg: "Membres ajoutés avec succès à la room.".to_string(),
                                    r#type: "add_members_clear".to_string(),
                                    message: format!("Des membres ont été ajoutés à la room {}", room_uuid_clone),
                                    name: None,
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    user_uuid: Some(user_uuid.clone()),
                                    json: None,
                                };
                                
                                // Notification à chaque invité
                                let manager = CONNECTION_MANAGER.lock().unwrap();
                                for invitee_uuid in &invitees_clone {
                                    let dm = DirectMessage {
                                        content: format!("Vous avez été ajouté à la room '{}'", room_uuid_clone),
                                        message_type: "room_added".to_string(),
                                        from_user: user_uuid.clone(),
                                        room_uuid: Some(room_uuid_clone.clone()),
                                        extra_data: None,
                                    };
                                    manager.send_direct_message(invitee_uuid, dm);
                                }
                                
                                // Récupérer le sender pour la room
                                let mut sender_opt = None;
                                if let Ok(room_uuid_parsed) = Uuid::parse_str(&room_uuid_clone) {
                                    let rooms_guard = rooms.lock().unwrap();
                                    sender_opt = rooms_guard.get(&room_uuid_parsed).cloned();
                                }
                                
                                (server_msg, sender_opt)
                            }
                            Err(e) => {
                                let error_msg = ServerMessage {
                                    msg: "Erreur lors de l'ajout des membres".to_string(),
                                    r#type: "error".to_string(),
                                    message: format!("Erreur BDD: {}", e),
                                    name: None,
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    user_uuid: Some(user_uuid.clone()),
                                    json: None,
                                };
                                (error_msg, None)
                            }
                        }
                    };
                    
                    ctx.spawn(
                        fut::wrap_future(fut).map(move |(server_msg, sender_opt), _, ctx: &mut WebsocketContext<MyWebSocket>| {
                            if let Some(sender) = sender_opt {
                                let _ = sender.send(server_msg.clone());
                            }
                            let json = serde_json::to_string(&server_msg).unwrap_or_default();
                            ctx.text(json);
                        }),
                    );
                }
                
                Ok(ClientMessage::DirectMessage { target_user, message }) => {
                    println!("Message direct: {} -> {}: {}", self.user_uuid, target_user, message);
                    
                    // Récupérer les informations de l'expéditeur
                    let pool = self.db_pool.clone();
                    let sender_uuid = self.user_uuid.clone();
                    
                    // Clone nécessaire pour move dans le future
                    let target_uuid = target_user.clone();
                    let message_content = message.clone();
                    
                    // Créer un futur pour traiter le message
                    let fut = async move {
                        // Récupérer le pseudo de l'expéditeur pour l'inclure dans le message
                        let sender_info = sqlx::query(
                            "SELECT pseudo FROM users WHERE uuid = ?")
                        .bind(sender_uuid)
                        .fetch_optional(&pool)
                        .await;
                        
                        let sender_name = match sender_info {
                            Ok(Some(row)) => row.get("pseudo"),
                            _ => "Utilisateur inconnu".to_string(),
                        };
                        
                        // Vérifier si l'utilisateur cible existe
                        let target_exists = sqlx::query_as(
                            "SELECT COUNT(*) as count FROM users WHERE uuid = ?")
                        .bind(target_uuid)
                        .fetch_one(&pool)
                        .await
                        .map(|row: (i32, )| row.0 > 0)
                        .unwrap_or(false);
                        
                        if !target_exists {
                            return ServerMessage {
                                msg: "Utilisateur introuvable".to_string(),
                                r#type: "error".to_string(),
                                message: "L'utilisateur cible n'existe pas".to_string(),
                                name: None,
                                room_uuid: None,
                                user_uuid: None,
                                json: None,
                            };
                        }
                        
                        // Enregistrer le message dans la base de données
                        let message_result = sqlx::query(
                            "INSERT INTO direct_messages (sender_uuid, receiver_uuid, content, sent_at) 
                             VALUES (?, ?, ?, datetime('now')) RETURNING id")
                        .bind(sender_uuid)
                        .bind(target_uuid)
                        .bind(message_content)
                        .fetch_one(&pool)
                        .await;
                        
                        let message_id = match message_result {
                            Ok(row) => row.get("id"),
                            Err(_) => 0,  // Valeur par défaut en cas d'erreur
                        };
                        
                        // Créer le message à envoyer
                        let dm = DirectMessage {
                            content: message_content,
                            message_type: "direct_message".to_string(),
                            from_user: sender_uuid,
                            room_uuid: None,
                            extra_data: Some(json!({
                                "sender_name": sender_name,
                                "message_id": message_id,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            })),
                        };
                        
                        // Envoyer le message
                        let manager = CONNECTION_MANAGER.lock().unwrap();
                        let delivered = manager.send_direct_message(&target_uuid, dm);
                        
                        // Message de confirmation pour l'expéditeur
                        ServerMessage {
                            msg: if delivered { "Message envoyé".to_string() } else { "Message enregistré, mais utilisateur hors ligne".to_string() },
                            r#type: "direct_message_status".to_string(),
                            message: message_content,
                            name: None,
                            room_uuid: None,
                            user_uuid: Some(target_uuid),
                            json: Some(json!({
                                "delivered": delivered,
                                "message_id": message_id,
                                "timestamp": chrono::Utc::now().to_rfc3339()
                            })),
                        }
                    };
                    
                    // Exécuter le futur
                    ctx.spawn(
                        fut::wrap_future(fut).map(|server_msg, _, ctx: &mut WebsocketContext<MyWebSocket>| {
                            if let Ok(json) = serde_json::to_string(&server_msg) {
                                ctx.text(json);
                            }
                        }),
                    );
                }
                
                // Ajout d'un nouveau type de message pour récupérer les messages directs non lus
                Ok(ClientMessage::GetUnreadMessages {}) => {
                    let pool = self.db_pool.clone();
                    let user_uuid = self.user_uuid.clone();
                    
                    let fut = async move {
                        // Récupérer les messages non lus pour cet utilisateur
                        let unread_msgs = sqlx::query(
                            "SELECT dm.id, dm.sender_uuid, dm.content, dm.sent_at, u.pseudo as sender_name
                             FROM direct_messages dm
                             JOIN users u ON u.uuid = dm.sender_uuid
                             WHERE dm.receiver_uuid = ? AND dm.read_at IS NULL
                             ORDER BY dm.sent_at DESC")
                            .bind(user_uuid)
                        .fetch_all(&pool)
                        .await;
                        
                        match unread_msgs {
                            Ok(messages) => {
                                // Convertir les messages en format JSON
                                let messages_json: Vec<serde_json::Value> = messages.iter().map(|msg| {
                                    json!({
                                        "id": msg.get("id"),
                                        "sender_uuid": msg.get("sender_uuid"),
                                        "sender_name": msg.get("sender_name"),
                                        "content": msg.get("content"),
                                        "sent_at": msg.get("sent_at")
                                    })
                                }).collect();
                                
                                // Marquer les messages comme lus
                                let _ = sqlx::query(
                                    "UPDATE direct_messages SET read_at = datetime('now')
                                     WHERE receiver_uuid = ? AND read_at IS NULL")
                                    .bind(user_uuid)
                                    .execute(&pool)
                                    .await;
                                
                                ServerMessage {
                                    msg: format!("{} messages non lus", messages.len()),
                                    r#type: "unread_messages".to_string(),
                                    message: format!("{} messages non lus", messages.len()),
                                    name: None,
                                    room_uuid: None,
                                    user_uuid: None,
                                    json: Some(json!({
                                        "messages": messages_json,
                                        "count": messages.len()
                                    })),
                                }
                            },
                            Err(e) => ServerMessage {
                                msg: "Erreur lors de la récupération des messages".to_string(),
                                r#type: "error".to_string(),
                                message: format!("Erreur BDD: {}", e),
                                name: None,
                                room_uuid: None,
                                user_uuid: None,
                                json: None,
                            },
                        }
                    };
                    
                    ctx.spawn(
                        fut::wrap_future(fut).map(|server_msg, _, ctx: &mut WebsocketContext<MyWebSocket>| {
                            if let Ok(json) = serde_json::to_string(&server_msg) {
                                ctx.text(json);
                            }
                        }),
                    );
                }
                
                Err(err) => {
                    eprintln!("Erreur de parsing WebSocket JSON: {}", err);
                    ctx.text("Erreur : message JSON invalide.");
                }
                
                _ => {} // Autres types de messages
            }
        }
    }
}
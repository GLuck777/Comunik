use actix_session::Session;
use actix::{Actor, StreamHandler};
use actix::AsyncContext;
use actix::fut; // pour wrap_future
use actix::ActorFutureExt;
use crate::other_ws::WebsocketContext;

use actix_web_actors::ws;
use serde::{Deserialize, Serialize};
use sqlx::{SqlitePool};
use crate::sql_req::*; // Assurez-vous que la fonction est correctement importée
use tokio::spawn;
use serde_json::Value;

use serde_json::json; // pour la recuperation du message unique à envoyer

// test CHANTIER
use tokio::sync::broadcast;
use std::collections::HashMap;
use uuid::Uuid;
use std::sync::Arc;
use std::sync::Mutex;

struct Server {
    rooms: Arc<Mutex<HashMap<Uuid, broadcast::Sender<ServerMessage>>>>,

}

use actix::Message;

#[derive(Message)]
#[rtype(result = "()")]
pub struct MyMessage(pub String);


use actix::Handler;

impl Handler<MyMessage> for MyWebSocket {
    type Result = ();

    fn handle(&mut self, msg: MyMessage, ctx: &mut Self::Context) {
        ctx.text(msg.0);
    }
}

// use actix::prelude::*;
// use std::collections::HashMap;
// Un acteur pour chaque participant
// struct WebSocketSession {
//     room: Addr<Room>, // L'adresse de la room où l'utilisateur est connecté
// }
// impl Actor for WebSocketSession {
//     type Context = ws::WebsocketContext<Self>;
// }
// impl WebSocketSession {
//     fn send_message_to_room(&self, message: String) {
//         // Envoie un message à tous les clients dans la room
//         self.room.do_send(RoomMessage(message));
//     }
// }
// // Un acteur pour gérer la room
// struct Room {
//     participants: HashMap<usize, Addr<WebSocketSession>>, // Liste des participants par ID
// }
// impl Actor for Room {
//     type Context = Context<Self>;
// }
// impl Room {
//     fn send_message(&self, message: String) {
//         // Envoie le message à tous les participants de la room
//         for participant in self.participants.values() {
//             participant.do_send(ws::Text(message.clone())); // Envoi du message à chaque participant
//         }
//     }
// }
// // Message envoyé à la room
// struct RoomMessage(pub String);
// impl Message for RoomMessage {
//     type Result = ();
// }
// impl Handler<RoomMessage> for Room {
//     type Result = ();
//     fn handle(&mut self, msg: RoomMessage, _: &mut Self::Context) {
//         self.send_message(msg.0);
//     }
// }

// L'envoi du message :
// Lorsqu'un utilisateur envoie un message, tu l'envoies à la room, puis la room envoie ce message à tous les autres participants. Cela peut se faire comme suit :
// Envoie un message à la room
// let room_addr = /* adresse de la room */;
// room_addr.do_send(RoomMessage("Un nouveau message dans la room!".to_string()));

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    #[serde(rename = "create_room")]
    CreateRoom { name: String, visibility: String },

    #[serde(rename = "chat")]
    Chat { room_uuid: String, message: String },

    #[serde(rename = "join_room")]
    GetRoomData { room_uuid: String },

    // À étendre : join_room, invite_user, etc.
}

#[derive(Debug, Serialize, Clone)]
pub struct ServerMessage {
    pub msg: String,
    pub r#type: String,   // `r#type` car `type` est un mot réservé en Rust
    pub message: String,
    // pub message: Value,
    pub name: Option<String>,      // facultatif si tu veux passer plus d'infos
    pub room_uuid: Option<String>, // par exemple
    pub user_uuid: Option<String>,
    pub json: Option<Value>
}

#[derive(Debug)]
pub struct MyWebSocket {
    pub db_pool: Arc<Mutex<SqlitePool>>,
    pub user_uuid: String,
    pub room_uuid: Option<String>, // <- new
    pub rooms: Arc<Mutex<HashMap<Uuid, broadcast::Sender<ServerMessage>>>>,
}
pub struct BroadcastMessage {
    pub room_uuid: String,
    pub message: String,
    pub from_user: String,
}


impl Actor for MyWebSocket {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for MyWebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        println!("websocket start!");
        if let Ok(ws::Message::Text(text)) = msg {
            match serde_json::from_str::<ClientMessage>(&text) {
                Ok(ClientMessage::CreateRoom { name, visibility }) => {
                    println!("create_room, name: {}", name);
                    
                    // Clone necessary data
                    let pool = Arc::clone(&self.db_pool);
                    let uuid = self.user_uuid.clone();
                    let name_clone = name.clone();
                    let visibility_clone = visibility.clone();
                    let rooms = Arc::clone(&self.rooms);
                    
                    // Create a future for room creation
                    let fut = async move {
                        // Get clean database connection
                        let db_pool = {
                            let guard = pool.lock().unwrap();
                            guard.clone() // Clone the pool, not the guard
                        };
                        
                        // Try to create the room
                        let result = create_room_in_db(&db_pool, &name_clone, &visibility_clone, &uuid, &[]).await;
                        
                        match result {
                            Ok(room_uuid) => {
                                // Try to parse the UUID
                                match Uuid::parse_str(&room_uuid) {
                                    Ok(room_uuid_parsed) => {
                                        // Create a broadcast channel for this room
                                        let (tx, _rx) = broadcast::channel(100);
                                        
                                        // Add to room mapping
                                        {
                                            let mut rooms_guard = rooms.lock().unwrap();
                                            rooms_guard.insert(room_uuid_parsed, tx);
                                        }
                                        
                                        // Create success message
                                        ServerMessage {
                                            msg: format!("Room '{}' créée avec succès par {}", name_clone, uuid),
                                            r#type: "room_created".to_string(),
                                            message: format!("Room '{}' créée avec succès par {}", name_clone, uuid),
                                            name: Some(name_clone),
                                            room_uuid: Some(room_uuid),
                                            user_uuid: Some(uuid),
                                            json: None,
                                        }
                                    },
                                    Err(_) => ServerMessage {
                                        msg: "Erreur de format UUID pour la room créée".to_string(),
                                        r#type: "error".to_string(),
                                        message: "Le format de l'UUID retourné est invalide".to_string(),
                                        name: None,
                                        room_uuid: None,
                                        user_uuid: None,
                                        json: None
                                    },
                                }
                            },
                            Err(e) => ServerMessage {
                                msg: "Erreur lors de la création de la room.".to_string(),
                                r#type: "error".to_string(),
                                message: format!("Database error: {}", e),
                                name: None,
                                room_uuid: None,
                                user_uuid: None,
                                json: None
                            },
                        }
                    };
                    
                    // Handle the future
                    ctx.spawn(
                        fut::wrap_future(fut).map(|msg, _, ctx: &mut WebsocketContext<MyWebSocket>| {
                            let json = serde_json::to_string(&msg).unwrap_or_default();
                            ctx.text(json);
                        }),
                    );
                }
                Ok(ClientMessage::GetRoomData { room_uuid }) => {
                    println!("Récupération de la room et des messages: {:?}", room_uuid);
                    
                    // Clone necessary data
                    let pool = Arc::clone(&self.db_pool);
                    let ctx_addr = ctx.address();
                    let room_uuid_clone = room_uuid.clone();
                    let rooms = Arc::clone(&self.rooms);
                    
                    // Set the room UUID in the websocket actor state
                    self.room_uuid = Some(room_uuid.clone());
                    
                    // Create a future to handle room data retrieval
                    let fut = async move {
                        // Get a clean database connection from the pool
                        let db_pool = {
                            let guard = pool.lock().unwrap();
                            guard.clone() // Clone the pool, not the guard
                        };
                        
                        // Try to parse the UUID
                        match Uuid::parse_str(&room_uuid_clone) {
                            Ok(room_uuid_parsed) => {
                                // Check if the room exists in our mapping
                                let sender_opt = {
                                    let rooms_guard = rooms.lock().unwrap();
                                    rooms_guard.get(&room_uuid_parsed).cloned()
                                };
                                
                                // Get room and message data
                                match get_room_with_messages(&db_pool, &room_uuid_clone).await {
                                    Ok((room, messages)) => {
                                        let message_json = serde_json::to_value(&messages).unwrap_or(json!([]));
                                        let server_msg = ServerMessage {
                                            msg: "Room et messages récupérés.".to_string(),
                                            r#type: "room_with_messages".to_string(),
                                            message: "".to_string(),
                                            name: Some(room.name),
                                            room_uuid: Some(room_uuid_clone.clone()),
                                            user_uuid: None,
                                            json: Some(message_json)
                                        };
                                        
                                        (room_uuid_parsed, server_msg, true, sender_opt)
                                    },
                                    Err(e) => {
                                        println!("Erreur récupération room/messages: {:?}", e);
                                        let error_msg = ServerMessage {
                                            msg: "Failed to retrieve room data".to_string(),
                                            r#type: "error".to_string(),
                                            message: format!("Database error: {}", e),
                                            name: None,
                                            room_uuid: Some(room_uuid_clone),
                                            user_uuid: None,
                                            json: None
                                        };
                                        
                                        (room_uuid_parsed, error_msg, false, sender_opt)
                                    }
                                }
                            },
                            Err(_) => {
                                let error_msg = ServerMessage {
                                    msg: "Invalid room UUID".to_string(),
                                    r#type: "error".to_string(),
                                    message: "The provided room UUID is invalid".to_string(),
                                    name: None,
                                    room_uuid: Some(room_uuid_clone),
                                    user_uuid: None,
                                    json: None
                                };
                                
                                // Return a dummy UUID since the parse failed
                                (Uuid::nil(), error_msg, false, None)
                            }
                        }
                    };
                    
                    // Handle the future
                    ctx.spawn(
                        fut::wrap_future(fut)
                            .map(move |(room_uuid_parsed, server_msg, success, sender_opt), _, ctx: &mut WebsocketContext<MyWebSocket> | {
                                // Send the response to the client
                                let json = serde_json::to_string(&server_msg).unwrap_or_default();
                                ctx.text(json);
                                
                                // If successful and we have a sender, set up room subscription
                                if success {
                                    if let Some(sender) = sender_opt {
                                        // Subscribe to the room's broadcast channel
                                        let mut rx = sender.subscribe();
                                        let addr = ctx.address();
                                        
                                        // Set up a separate task to listen for room messages
                                        actix::spawn(async move {
                                            while let Ok(msg) = rx.recv().await {
                                                let json = serde_json::to_string(&msg).unwrap_or_default();
                                                let _ = addr.send(MyMessage(json)).await;
                                            }
                                        });
                                    }
                                }
                            }),
                    );
                }

                // Ok(ClientMessage::Chat { room_uuid, message }) => {
                //     println!("Message dans room {:?}: {}", room_uuid, message);
                //     let pool = self.db_pool.clone();
                //     let uuid = self.user_uuid.clone();
                //     // 💡 vérifie si self.uuid est membre de la room avant d'enregistrer
                //     if is_user_in_room(&pool, &room_uuid, &uuid).await? {
                //         insert_message(&pool, &room_uuid, &uuid, &message).await?;
                //         // éventuellement broadcast aux autres
                //     } else {
                //         println!("User {:?} n'est pas membre de la room {:?}", uuid, room_uuid);
                //         // tu peux aussi renvoyer un message d'erreur au client
                //     }
                // }

                Ok(ClientMessage::Chat { room_uuid, message }) => {
                    println!("Message dans room {:?}: {}", room_uuid, message);
                
                    // Clone all necessary data before moving into async context
                    let pool = Arc::clone(&self.db_pool);
                    let uuid = self.user_uuid.clone();
                    let room_uuid_clone = room_uuid.clone();
                    
                    // Parse UUID once before the lock
                    match Uuid::parse_str(&room_uuid) {
                        Ok(room_uuid_parsed) => {
                            // Get the sender outside of the async block
                            let sender_opt = {
                                let rooms = self.rooms.lock().unwrap();
                                rooms.get(&room_uuid_parsed).cloned()
                            };
                
                            // Create a future to handle the database operations
                            let fut = async move {
                                // Important: Lock the pool briefly just to get a connection
                                let db_pool = {
                                    let guard = pool.lock().unwrap();
                                    guard.clone() // Clone the pool itself, not the guard
                                };
                                
                                // Check if user is in room - with proper error handling
                                let is_member = match is_user_in_room(&db_pool, &room_uuid_clone, &uuid).await {
                                    Ok(result) => result,
                                    Err(e) => {
                                        eprintln!("Error checking room membership: {:?}", e);
                                        return ServerMessage {
                                            msg: "Error checking room membership".into(),
                                            r#type: "error".into(),
                                            message: format!("Failed to verify membership: {}", e),
                                            name: None,
                                            room_uuid: Some(room_uuid_clone),
                                            user_uuid: Some(uuid),
                                            json: None,
                                        };
                                    }
                                };
                
                                if !is_member {
                                    return ServerMessage {
                                        msg: "Not a member of this room".into(),
                                        r#type: "error".into(),
                                        message: "You are not a member of this room".into(),
                                        name: None,
                                        room_uuid: Some(room_uuid_clone),
                                        user_uuid: Some(uuid),
                                        json: None,
                                    };
                                }
                
                                // Insert message
                                if let Err(e) = insert_message(&db_pool, &room_uuid_clone, &uuid, &message).await {
                                    eprintln!("Error inserting message: {:?}", e);
                                    return ServerMessage {
                                        msg: "Failed to save message".into(),
                                        r#type: "error".into(),
                                        message: format!("Database error: {}", e),
                                        name: None,
                                        room_uuid: Some(room_uuid_clone),
                                        user_uuid: Some(uuid),
                                        json: None,
                                    };
                                }
                
                                // Get user name
                                let user_name = match get_user_name(&db_pool, &uuid).await {
                                    Ok(name) => name,
                                    Err(_) => "Anonymous".into(),
                                };
                
                                let created_at = chrono::Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
                
                                // Create the message to broadcast
                                ServerMessage {
                                    msg: "Nouveau message".into(),
                                    r#type: "new_message".into(),
                                    message: message.clone(),
                                    name: Some(user_name.clone()),
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    user_uuid: Some(uuid.clone()),
                                    json: Some(json!({
                                        "content": message,
                                        "created_at": created_at,
                                        "user_name": user_name,
                                        "room_uuid": room_uuid_clone,
                                        "user_uuid": uuid,
                                    })),
                                }
                            };
                
                            // Handle the future result
                            ctx.spawn(
                                fut::wrap_future(fut)
                                    .map(move |server_msg, _, ctx: &mut WebsocketContext<MyWebSocket> | {
                                        // If there's an error message, just send it to the requesting user
                                        if server_msg.r#type == "error" {
                                            let json = serde_json::to_string(&server_msg).unwrap_or_default();
                                            ctx.text(json);
                                            return;
                                        }
                
                                        // If successful, broadcast to all users in the room
                                        if let Some(sender) = sender_opt {
                                            let _ = sender.send(server_msg.clone());
                                        }
                
                                        // Also confirm to the sender
                                        let json = serde_json::to_string(&server_msg).unwrap_or_default();
                                        ctx.text(json);
                                    }),
                            );
                        },
                        Err(_) => {
                            let error_msg = ServerMessage {
                                msg: "Invalid room UUID format".into(),
                                r#type: "error".into(),
                                message: "The provided room UUID is invalid".into(),
                                name: None,
                                room_uuid: Some(room_uuid),
                                user_uuid: Some(self.user_uuid.clone()),
                                json: None,
                            };
                            let json = serde_json::to_string(&error_msg).unwrap_or_default();
                            ctx.text(json);
                        }
                    }
                }
                // _ => {}
                Err(err) => {
                    eprintln!("Erreur de parsing WebSocket JSON: {}", err);
                    ctx.text("Erreur : message JSON invalide.");
                }
            }
        }
    }
}


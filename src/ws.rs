
use crate::connection_manager::CONNECTION_MANAGER;
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
// use tokio::sync::broadcast;
use std::collections::HashMap;
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

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

    #[serde(rename = "delete_member")]
    DeleteRoomMembers {room_uuid: String, member_uuid: String},
    
    #[serde(rename = "change_room_name")]
    UpdateRoomName {room_uuid: String, content: String},
    
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
        let addr = ctx.address();
        let user_uuid = self.user_uuid.clone();

        actix::spawn(async move {
            let mut manager = CONNECTION_MANAGER.lock().await;
            let user_data = manager.user_data.remove(&user_uuid);
            manager.register_connection(user_uuid, addr, user_data);
        });
    }

    fn stopped(&mut self, _: &mut Self::Context) {
        let user_uuid = self.user_uuid.clone();
        actix::spawn(async move {
            let mut manager = CONNECTION_MANAGER.lock().await;
            manager.unregister_connection(&user_uuid);
        });
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
                                            let mut rooms_guard = rooms.lock().await;
                                            rooms_guard.insert(room_uuid_parsed, tx.clone());
                                            
                                            // Aussi mettre à jour le gestionnaire global de connexions
                                            let mut manager = CONNECTION_MANAGER.lock().await;
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
                                        let manager = CONNECTION_MANAGER.lock().await;
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
                
                Ok(ClientMessage::GetRoomData { room_uuid }) => {
                    println!("\n\nRécupération de la room et des messages: {:?}\n\n", room_uuid);
                    
                    // Clone necessary data
                    // let pool: Arc<Mutex<SqlitePool>> = Arc::clone(&self.db_pool);
                    let pool = self.db_pool.clone();
                    let ctx_addr = ctx.address();
                    let room_uuid_clone = room_uuid.clone();
                    let rooms = Arc::clone(&self.rooms);
                    
                    // Set the room UUID in the websocket actor state
                    self.room_uuid = Some(room_uuid.clone());
                    
                    // Create a future to handle room data retrieval
                    let fut = async move {
                        // Get a clean database connection from the pool
                        // let db_pool = {
                        //     let guard = pool.lock().unwrap();
                        //     guard.clone() // Clone the pool, not the guard
                        // };
                        // let db_pool = pool.lock().expect("Failed to lock db_pool").clone();

                        
                        // Try to parse the UUID
                        match Uuid::parse_str(&room_uuid_clone) {
                            Ok(room_uuid_parsed) => {
                                // Check if the room exists in our mapping
                                let sender_opt: Option<broadcast::Sender<ServerMessage>> = {
                                    let rooms_guard = rooms.lock().await;
                                    rooms_guard.get(&room_uuid_parsed).cloned()
                                };
                                
                                // Get room and message data
                                match get_room_with_messages(&pool, &room_uuid_clone).await {
                                    
                                    Ok((room, messages, member_uuids)) => {
                                        // let message_json = serde_json::to_value(&messages).unwrap_or(json!([]));
                                        let message_json = json!({
                                            "messages": messages,
                                            "member_uuids": member_uuids
                                        });
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
                
                Ok(ClientMessage::Chat { room_uuid, message }) => {
                    println!("Message dans room {:?}: {}", room_uuid, message);
                
                    // Clone all necessary data before moving into async context
                    // let pool: Arc<Mutex<SqlitePool>> = Arc::clone(&self.db_pool);
                    let pool = self.db_pool.clone();
                    let uuid = self.user_uuid.clone();
                    let room_uuid_clone = room_uuid.clone();
                    
                    // Parse UUID once before the lock
                    match Uuid::parse_str(&room_uuid) {
                        Ok(room_uuid_parsed) => {
                            let rooms = self.rooms.clone(); // clone pour use dans le async move
                                            
                            // Create a future to handle the database operations
                            let fut = async move {
                                // Important: Lock the pool briefly just to get a connection
                                // Get the sender outside of the async block
                                let sender_opt = {
                                    let rooms = rooms.lock().await; //moved here because need async
                                    rooms.get(&room_uuid_parsed).cloned()
                                };
                                
                                // Check if user is in room - with proper error handling
                                let is_member = match is_user_in_room(&pool, &room_uuid_clone, &uuid).await {
                                    Ok(result) => result,
                                    Err(e) => {
                                        eprintln!("Error checking room membership: {:?}", e);
                                        return (sender_opt, ServerMessage {
                                            msg: "Error checking room membership".into(),
                                            r#type: "error".into(),
                                            message: format!("Failed to verify membership: {}", e),
                                            name: None,
                                            room_uuid: Some(room_uuid_clone),
                                            user_uuid: Some(uuid),
                                            json: None,
                                        });
                                    }
                                };
                
                                if !is_member {
                                    return (sender_opt, ServerMessage {
                                        msg: "Not a member of this room".into(),
                                        r#type: "error".into(),
                                        message: "You are not a member of this room".into(),
                                        name: None,
                                        room_uuid: Some(room_uuid_clone),
                                        user_uuid: Some(uuid),
                                        json: None,
                                    });
                                }
                
                                // Insert message
                                if let Err(e) = insert_message(&pool, &room_uuid_clone, &uuid, &message).await {
                                    eprintln!("Error inserting message: {:?}", e);
                                    return (sender_opt, ServerMessage {
                                        msg: "Failed to save message".into(),
                                        r#type: "error".into(),
                                        message: format!("Database error: {}", e),
                                        name: None,
                                        room_uuid: Some(room_uuid_clone),
                                        user_uuid: Some(uuid),
                                        json: None,
                                    });
                                    
                                }
                
                                // Get user name
                                let user_name = match get_user_name(&pool, &uuid).await {
                                    Ok(name) => name,
                                    Err(_) => "Anonymous".into(),
                                };
                
                                let created_at = chrono::Utc::now().format("%d/%m/%Y %H:%M:%S").to_string();
                
                                // Create the message to broadcast
                                let server_msg = ServerMessage {
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
                                };
                                (sender_opt, server_msg)
                            };
                
                            // Handle the future result
                            ctx.spawn(
                                fut::wrap_future(fut)
                                    .map(move |(sender_opt, server_msg), _, ctx: &mut WebsocketContext<MyWebSocket> | {
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
                                        // let json = serde_json::to_string(&server_msg).unwrap_or_default();
                                        // ctx.text(json);
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
                
                Ok(ClientMessage::GetUsersData{}) => {
                    println!("\n\n\n\nRécupération des utilisateurs pour:\n\n\n\n");
                    let pool = self.db_pool.clone();
                    let user_uuid = self.user_uuid.clone();
                    let mut username = "Anonyme".to_string();

                    let fut = async move {

                        if user_uuid.clone() != "" {
                            if let Ok(name) = get_user_name(&pool, &user_uuid).await {
                                username = name;
                            }
                        };
                        match get_list_users(&pool, username).await {
                            Ok(list_users) => {
                                let message_json = serde_json::to_value(&list_users).unwrap_or(json!([]));
                                let server_msg = ServerMessage {
                                    msg: "Utilisateurs récupérés.".to_string(),
                                    r#type: "list_users".to_string(),
                                    message: "".to_string(),
                                    name: None,
                                    room_uuid: None,
                                    user_uuid: None,
                                    json: Some(message_json)
                                };
                                
                                server_msg
                                
                            },
                            Err(e) => {
                                println!("Erreur récupération room/messages: {:?}", e);
                                let error_msg = ServerMessage {
                                    msg: "Failed to retrieve room data".to_string(),
                                    r#type: "error".to_string(),
                                    message: format!("Database error: {}", e),
                                    name: None,
                                    room_uuid: None,
                                    user_uuid: None,
                                    json: None
                                };
                                error_msg
                            }
                        }
                    };
                    ctx.spawn(
                    fut::wrap_future(fut)
                    .map(move |server_msg, _, ctx: &mut WebsocketContext<MyWebSocket> | {
                            // Send the response to the client
                            if server_msg.r#type == "error" {
                                let json = serde_json::to_string(&server_msg).unwrap_or_default();
                                ctx.text(json);
                                return;
                            }
                            // Also confirm to the sender
                            let json = serde_json::to_string(&server_msg).unwrap_or_default();
                            ctx.text(json);
                        })
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
                        let result: Result<(Vec<(Uuid, String)>, Vec<Member>), sqlx::Error> = add_members_in_room(&pool, &room_uuid_clone, invitees_clone.clone()).await;
                    // get_room_with_messages, add_members_in_room
                        match result {
                            Ok((added_users, added_members)) => {
                                println!("\x1b[0;33m[Membres ajoutés dans la room]:\x1b[0m");
                                let members_info: Vec<_> = added_members.iter().map(|m| {
                                    json!({
                                        "uuid": m.uuid,
                                        "pseudo": m.pseudo,
                                        "owner": m.owner
                                    })
                                }).collect();

                                let message_json = json!({
                                    "members": members_info
                                });
                            // Ok(added_users) => {
                            //     println!("\x1b[0;33m[Membres ajoutés dans la room]:\x1b[0m");
                            //     for (uuid, pseudo) in &added_users {
                            //         println!("\x1b[0;33m - {} ({})\x1b[0m", pseudo, uuid);
                            //     }
                    
                                // Message pour celui qui a ajouté les membres
                                let server_msg = ServerMessage {
                                    msg: "Membres ajoutés avec succès à la room.".to_string(),
                                    r#type: "add_members_clear".to_string(),
                                    message: format!("Des membres ont été ajoutés à la room {}", room_uuid_clone),
                                    name: None,
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    user_uuid: Some(user_uuid.clone()),
                                    json: Some(message_json),
                                };
                                
                                // Notification à chaque invité
                                let manager = CONNECTION_MANAGER.lock().await;
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
                                    let rooms_guard = rooms.lock().await;
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

                Ok(ClientMessage::DeleteRoomMembers{room_uuid, member_uuid }) => {
                    println!("\x1b[0;31m DeleteRoomMembers! \x1b[0m");
                    let pool = self.db_pool.clone();
                    let user_uuid = self.user_uuid.clone();
                    let room_uuid_clone = room_uuid.clone();
                    let rooms = Arc::clone(&self.rooms);
                    // let rooms_clone = Arc::clone(&rooms);
                    
                    let fut = async move {
                        let result= delete_members_in_room(&pool, &room_uuid_clone, &member_uuid).await;
                    // get_room_with_messages, add_members_in_room
                        match result {
                            Ok(list_members) => {
                                println!("\x1b[0;33m[Membres ajoutés dans la room]:\x1b[0m");
                                let members_info: Vec<_> = list_members.iter().map(|m| {
                                    json!({
                                        "uuid": m.uuid,
                                        "pseudo": m.pseudo,
                                        "owner": m.owner
                                    })
                                }).collect();

                                let message_json = json!({
                                    "members": members_info
                                });                    
                                
                                // Get room name
                                let room_name = match get_room_name(&pool, &room_uuid_clone).await {
                                    Ok(name) => name,
                                    Err(_) => room_uuid_clone.clone(),
                                };

                                // Message pour celui qui a ajouté les membres
                                let server_msg = ServerMessage {
                                    msg: "Membres supprimé avec succès à la room.".to_string(),
                                    r#type: "suppr_members_clear".to_string(),
                                    message: format!("Des membres ont été ajoutés à la room {}", room_name),
                                    name: None,
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    user_uuid: Some(user_uuid.clone()),
                                    json: Some(message_json),
                                };
                                
                                // Notification à chaque invité
                                let manager = CONNECTION_MANAGER.lock().await;
                                
                                
                                //envoi à la personne concerné par la suppression l'information de sa suppression 
                                let dm = DirectMessage {
                                    content: format!("Vous avez été supprimer à la room '{}'", room_name),
                                    message_type: "suppr_from_room".to_string(),
                                    from_user: user_uuid.clone(),
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    extra_data: None,
                                };
                                manager.send_direct_message(&member_uuid, dm);
                                // fin du message direct vers le member_uuid
                                
                                // Récupérer le sender pour la room
                                let mut sender_opt = None;
                                if let Ok(room_uuid_parsed) = Uuid::parse_str(&room_uuid_clone) {
                                    let rooms_guard = rooms.lock().await;
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
                Ok(ClientMessage::UpdateRoomName{room_uuid, content }) => {
                    println!("\x1b[0;31m editRoomName ! \x1b[0m");
                    let pool = self.db_pool.clone();
                    let user_uuid = self.user_uuid.clone();
                    let room_uuid_clone = room_uuid.clone();
                    let rooms = Arc::clone(&self.rooms);
                    // let rooms_clone = Arc::clone(&rooms);
                    
                    let fut = async move {
                        // Get room name
                        let old_room_name = match get_room_name(&pool, &room_uuid_clone).await {
                            Ok(name) => name,
                            Err(_) => room_uuid_clone.clone(),
                        };
                        let result= update_room_name(&pool, &room_uuid_clone, &content).await;
                    // get_room_with_messages, add_members_in_room
                        match result {
                            Ok(new_room_name) => {
                                println!("\x1b[0;33m[Membres ajoutés dans la room]:\x1b[0m");

                                // Message pour celui qui a ajouté les membres
                                let server_msg = ServerMessage {
                                    msg: "Nom de la room changé avec succès.".to_string(),
                                    r#type: "name_change_clear".to_string(),
                                    message: format!("Le nom de la room {} à été changé par {}", old_room_name, new_room_name),
                                    name: Some(new_room_name),
                                    room_uuid: Some(room_uuid_clone.clone()),
                                    user_uuid: Some(user_uuid.clone()),
                                    json: None,
                                };
                                
                                // Récupérer le sender pour la room
                                let mut sender_opt = None;
                                if let Ok(room_uuid_parsed) = Uuid::parse_str(&room_uuid_clone) {
                                    let rooms_guard = rooms.lock().await;
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
                
                Err(err) => {
                    eprintln!("Erreur de parsing WebSocket JSON: {}", err);
                    ctx.text("Erreur : message JSON invalide.");
                }
                
                _ => {} // Autres types de messages
            }
        }
    }
}
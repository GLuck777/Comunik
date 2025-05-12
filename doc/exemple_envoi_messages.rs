// Utilitaires pour l'envoi de messages basés sur l'UUID

// Cette fonction pourrait être dans un module services ou utils
pub async fn send_message_to_users(
    user_uuids: Vec<String>,
    message: &str,
    message_type: &str,
    from_user: &str,
    app_state: &web::Data<AppState>,
) -> Vec<String> {
    // Créer le message direct
    let dm = DirectMessage {
        content: message.to_string(),
        message_type: message_type.to_string(),
        from_user: from_user.to_string(),
        room_uuid: None,
        extra_data: None,
    };
    
    // Option 1: Avec ConnectionManager global
    let failed_users = {
        let manager = CONNECTION_MANAGER.lock().unwrap();
        manager.broadcast_to_users(&user_uuids, dm)
    };
    
    // Option 2: Avec AppState
    /*
    let failed_users = {
        let manager = app_state.connection_manager.lock().await;
        manager.broadcast_to_users(&user_uuids, dm)
    };
    */
    
    failed_users
}

// Exemple: Fonction pour envoyer un message à tous les membres d'un groupe
pub async fn send_message_to_group_members(
    group_id: &str,
    message: &str,
    from_user: &str,
    app_state: &web::Data<AppState>,
) -> Result<Vec<String>, sqlx::Error> {
    // Récupérer les membres du groupe depuis la base de données
    let members = sqlx::query_as::<_, (String,)>(
        "SELECT user_uuid FROM group_members WHERE group_id = ?"
    )
    .bind(group_id)
    .fetch_all(&app_state.db_pool)
    .await?;
    
    // Extraire les UUIDs
    let member_uuids: Vec<String> = members.into_iter().map(|(uuid,)| uuid).collect();
    
    // Envoyer le message
    let failed_users = send_message_to_users(
        member_uuids,
        message,
        "group_message",
        from_user,
        app_state
    ).await;
    
    Ok(failed_users)
}

// Exemple: Fonction pour envoyer un message aux administrateurs
pub async fn send_message_to_admins(
    message: &str,
    from_user: &str,
    app_state: &web::Data<AppState>,
) -> Result<Vec<String>, sqlx::Error> {
    // Récupérer les administrateurs depuis la base de données
    let admins = sqlx::query_as::<_, (String,)>(
        "SELECT uuid FROM users WHERE role = 'admin'"
    )
    .fetch_all(&app_state.db_pool)
    .await?;
    
    // Extraire les UUIDs
    let admin_uuids: Vec<String> = admins.into_iter().map(|(uuid,)| uuid).collect();
    
    // Envoyer le message
    let failed_users = send_message_to_users(
        admin_uuids,
        message,
        "admin_notification",
        from_user,
        app_state
    ).await;
    
    Ok(failed_users)
}

// Exemple: Fonction pour envoyer un message aux utilisateurs connectés d'une liste
pub async fn send_message_to_online_users(
    user_uuids: Vec<String>,
    message: &str,
    from_user: &str,
    app_state: &web::Data<AppState>,
) -> Vec<String> {
    // Filtrer pour ne garder que les utilisateurs connectés
    let online_uuids = {
        let manager = CONNECTION_MANAGER.lock().unwrap();
        user_uuids.into_iter()
            .filter(|uuid| manager.is_user_connected(uuid))
            .collect::<Vec<String>>()
    };
    
    // Envoyer le message
    send_message_to_users(
        online_uuids,
        message,
        "notification",
        from_user,
        app_state
    ).await
}

// Handler exemple pour envoyer un message à tous les utilisateurs d'une room
pub async fn notify_room_members(
    path: web::Path<String>, // UUID de la room
    body: web::Json<NotificationData>,
    app_state: web::Data<AppState>,
) -> Result<HttpResponse, actix_web::Error> {
    let room_uuid = path.into_inner();
    
    // Récupérer les membres de la room
    let members = sqlx::query_as::<_, (String,)>(
        "SELECT user_uuid FROM room_members WHERE room_uuid = ?"
    )
    .bind(&room_uuid)
    .fetch_all(&app_state.db_pool)
    .await
    .map_err(|e| {
        actix_web::error::ErrorInternalServerError(format!("Database error: {}", e))
    })?;
    
    // Extraire les UUIDs
    let member_uuids: Vec<String> = members.into_iter().map(|(uuid,)| uuid).collect();
    
    // Envoyer le message
    let failed_users = send_message_to_users(
        member_uuids.clone(),
        &body.message,
        "room_notification",
        "system",
        &app_state
    ).await;
    
    Ok(HttpResponse::Ok().json(json!({
        "success": true,
        "total_members": member_uuids.len(),
        "delivered": member_uuids.len() - failed_users.len(),
        "failed": failed_users
    })))
}


//

                // Ok(ClientMessage::DirectMessage { target_user, message }) => {
                //     println!("Message direct: {} -> {}: {}", self.user_uuid, target_user, message);
                    
                //     // Récupérer les informations de l'expéditeur
                //     let pool = self.db_pool.clone();
                //     let sender_uuid = self.user_uuid.clone();
                    
                //     // Clone nécessaire pour move dans le future
                //     let target_uuid = target_user.clone();
                //     let message_content = message.clone();
                    
                //     // Créer un futur pour traiter le message
                //     let fut = async move {
                //         // Récupérer le pseudo de l'expéditeur pour l'inclure dans le message
                //         let sender_info = sqlx::query(
                //             "SELECT pseudo FROM users WHERE uuid = ?")
                //         .bind(sender_uuid)
                //         .fetch_optional(&pool)
                //         .await;
                        
                //         let sender_name = match sender_info {
                //             Ok(Some(row)) => row.get("pseudo"),
                //             _ => "Utilisateur inconnu".to_string(),
                //         };
                        
                //         // Vérifier si l'utilisateur cible existe
                //         let target_exists = sqlx::query_as(
                //             "SELECT COUNT(*) as count FROM users WHERE uuid = ?")
                //         .bind(target_uuid)
                //         .fetch_one(&pool)
                //         .await
                //         .map(|row: (i32, )| row.0 > 0)
                //         .unwrap_or(false);
                        
                //         if !target_exists {
                //             return ServerMessage {
                //                 msg: "Utilisateur introuvable".to_string(),
                //                 r#type: "error".to_string(),
                //                 message: "L'utilisateur cible n'existe pas".to_string(),
                //                 name: None,
                //                 room_uuid: None,
                //                 user_uuid: None,
                //                 json: None,
                //             };
                //         }
                        
                //         // Enregistrer le message dans la base de données
                //         let message_result = sqlx::query(
                //             "INSERT INTO direct_messages (sender_uuid, receiver_uuid, content, sent_at) 
                //              VALUES (?, ?, ?, datetime('now')) RETURNING id")
                //         .bind(sender_uuid)
                //         .bind(target_uuid)
                //         .bind(message_content)
                //         .fetch_one(&pool)
                //         .await;
                        
                //         let message_id = match message_result {
                //             Ok(row) => row.get("id"),
                //             Err(_) => 0,  // Valeur par défaut en cas d'erreur
                //         };
                        
                //         // Créer le message à envoyer
                //         let dm = DirectMessage {
                //             content: message_content,
                //             message_type: "direct_message".to_string(),
                //             from_user: sender_uuid,
                //             room_uuid: None,
                //             extra_data: Some(json!({
                //                 "sender_name": sender_name,
                //                 "message_id": message_id,
                //                 "timestamp": chrono::Utc::now().to_rfc3339()
                //             })),
                //         };
                        
                //         // Envoyer le message
                //         let manager = CONNECTION_MANAGER.lock().await;
                //         let delivered = manager.send_direct_message(&target_uuid, dm);
                        
                //         // Message de confirmation pour l'expéditeur
                //         ServerMessage {
                //             msg: if delivered { "Message envoyé".to_string() } else { "Message enregistré, mais utilisateur hors ligne".to_string() },
                //             r#type: "direct_message_status".to_string(),
                //             message: message_content,
                //             name: None,
                //             room_uuid: None,
                //             user_uuid: Some(target_uuid),
                //             json: Some(json!({
                //                 "delivered": delivered,
                //                 "message_id": message_id,
                //                 "timestamp": chrono::Utc::now().to_rfc3339()
                //             })),
                //         }
                //     };
                    
                //     // Exécuter le futur
                //     ctx.spawn(
                //         fut::wrap_future(fut).map(|server_msg, _, ctx: &mut WebsocketContext<MyWebSocket>| {
                //             if let Ok(json) = serde_json::to_string(&server_msg) {
                //                 ctx.text(json);
                //             }
                //         }),
                //     );
                // }
                
                // Ajout d'un nouveau type de message pour récupérer les messages directs non lus
                // Ok(ClientMessage::GetUnreadMessages {}) => {
                //     let pool = self.db_pool.clone();
                //     let user_uuid = self.user_uuid.clone();
                    
                //     let fut = async move {
                //         // Récupérer les messages non lus pour cet utilisateur
                //         let unread_msgs = sqlx::query(
                //             "SELECT dm.id, dm.sender_uuid, dm.content, dm.sent_at, u.pseudo as sender_name
                //              FROM direct_messages dm
                //              JOIN users u ON u.uuid = dm.sender_uuid
                //              WHERE dm.receiver_uuid = ? AND dm.read_at IS NULL
                //              ORDER BY dm.sent_at DESC")
                //             .bind(user_uuid)
                //         .fetch_all(&pool)
                //         .await;
                        
                //         match unread_msgs {
                //             Ok(messages) => {
                //                 // Convertir les messages en format JSON
                //                 let messages_json = messages.iter().map(|msg| {
                //                     json!({ 
                //                     // "id": msg.get("id"),
                //                     //     "sender_uuid": msg.get("sender_uuid"),
                //                     //     "sender_name": msg.get("sender_name"),
                //                     //     "content": msg.get("content"),
                //                     //     "sent_at": msg.get("sent_at") 
                //                     })
                //                 }).collect::<Vec<_>>();
                                
                //                 // Marquer les messages comme lus
                //                 let _ = sqlx::query(
                //                     "UPDATE direct_messages SET read_at = datetime('now')
                //                      WHERE receiver_uuid = ? AND read_at IS NULL")
                //                     .bind(user_uuid)
                //                     .execute(&pool)
                //                     .await;
                                
                //                 ServerMessage {
                //                     msg: format!("{} messages non lus", messages.len()),
                //                     r#type: "unread_messages".to_string(),
                //                     message: format!("{} messages non lus", messages.len()),
                //                     name: None,
                //                     room_uuid: None,
                //                     user_uuid: None,
                //                     json: Some(json!({
                //                         "messages": messages_json,
                //                         "count": messages.len()
                //                     })),
                //                 }
                //             },
                //             Err(e) => ServerMessage {
                //                 msg: "Erreur lors de la récupération des messages".to_string(),
                //                 r#type: "error".to_string(),
                //                 message: format!("Erreur BDD: {}", e),
                //                 name: None,
                //                 room_uuid: None,
                //                 user_uuid: None,
                //                 json: None,
                //             },
                //         }
                //     };
                    
                //     ctx.spawn(
                //         fut::wrap_future(fut).map(|server_msg, _, ctx: &mut WebsocketContext<MyWebSocket>| {
                //             if let Ok(json) = serde_json::to_string(&server_msg) {
                //                 ctx.text(json);
                //             }
                //         }),
                //     );
                // }
                
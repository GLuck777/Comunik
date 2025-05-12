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
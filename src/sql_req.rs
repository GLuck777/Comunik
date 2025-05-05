use sqlx::{SqlitePool, Row, FromRow, Error};
use uuid::Uuid;
use serde::Serialize;
use chrono::NaiveDateTime;

pub async fn create_room_in_db(
    pool: &SqlitePool,
    name: &str,
    visibility: &str,
    owner_uuid: &str,
    invitees: &[String], // Vec<String>
) -> Result<String, Error> {
    let room_uuid = Uuid::new_v4().to_string();
    println!(
        "room uuid: {}, proprio: {} visibility: {}, name: {}",
        room_uuid, owner_uuid, visibility, name
    );

    // Insère la room
    let result = sqlx::query("INSERT INTO rooms (room_uuid, name, owner_uuid) VALUES (?, ?, ?)")
        .bind(&room_uuid)
        .bind(name)
        .bind(owner_uuid)
        .execute(pool)
        .await;

    if let Err(e) = result {
        println!("Erreur lors de l'insertion dans rooms: {:?}", e);
        return Err(e);
    }

    // Ajoute le créateur comme membre
    let result = sqlx::query(
        "INSERT INTO room_members (room_uuid, user_uuid, role) VALUES (?, ?, 'owner')",
    )
    .bind(&room_uuid)
    .bind(owner_uuid)
    .execute(pool)
    .await;

    if let Err(e) = result {
        println!("Erreur lors de l'insertion dans room_members: {:?}", e);
        return Err(e);
    }

    // Collecte les UUIDs pour les notifications
    let mut notification_receivers: Vec<String> = vec![owner_uuid.to_string()];

    println!("######################### invitées {:?}", invitees);
    for invitee in invitees {
        if let Some(row) = sqlx::query("SELECT uuid FROM users WHERE LOWER(email) = LOWER(?) OR LOWER(pseudo) = LOWER(?) OR LOWER(uuid) = LOWER(?)")
            .bind(invitee)
            .bind(invitee)
            .bind(invitee)
            .fetch_optional(pool)
            .await?
        {
            let user_uuid: String = row.get("uuid");

            // Ajoute en tant que membre invité
            let result = sqlx::query(
                "INSERT INTO room_members (room_uuid, user_uuid, role) VALUES (?, ?, 'invited')",
            )
            .bind(&room_uuid)
            .bind(&user_uuid)
            .execute(pool)
            .await;

            if let Err(e) = result {
                println!("\n\n\tErreur lors de l'insertion dans room_members pour les invitées!!!!: {:?}", e);
                return Err(e);
            }

            notification_receivers.push(user_uuid); // Ajoute l'invité à la liste de notifs
        }
    }


    let name_u = sqlx::query("SELECT pseudo FROM users WHERE uuid = ? LIMIT 1")
    .bind(owner_uuid)
    .fetch_one(pool)
    .await;
    let name_user = match name_u {
        Ok(row) => match row.try_get::<String, _>("pseudo") {
            Ok(pseudo) => pseudo,
            Err(e) => {
                println!("Erreur lors de l'accès au champ pseudo: {:?}", e);
                return Err(e.into());
            }
        },
        Err(e) => {
            println!("Erreur lors de la récupération du pseudo: {:?}", e);
            return Err(e.into());
        }
    };
    // Envoie les notifications à tous
    for receiver_uuid in notification_receivers {
        let result = sqlx::query(
            "INSERT INTO notifications (user_uuid, message) VALUES (?, ?)",
        )
        .bind(&receiver_uuid)
        .bind(format!("Room '{}' créée avec succès par {}", name, name_user))//{} owner_uuid
        .execute(pool)
        .await;

        if let Err(e) = result {
            println!(
                "Erreur lors de l'insertion dans notifications pour {}: {:?}",
                receiver_uuid, e
            );
        }
    }

    Ok(room_uuid)
}


#[derive(Serialize,FromRow, Debug)]
pub struct Message {
    pub id: i64,
    pub room_uuid: String,
    pub user_uuid: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Serialize, FromRow, Debug)]
pub struct MessageWithUser {
    pub id: i64,
    pub room_uuid: String,
    pub user_uuid: String,
    pub user_name: String,
    pub content: String,

    #[serde(serialize_with = "my_custom_date_serializer")]
    pub created_at: NaiveDateTime,
}

#[derive(Serialize,FromRow, Debug)]
pub struct Room {
    pub room_uuid: String,
    pub name: String,
    pub owner_uuid: String,
    //pub member_count: i64,  // Nombre de membres dans la salle
}
// Make sure your Room struct looks like this
#[derive(Debug, FromRow, Serialize)]
pub struct RoomSpe {
    pub room_uuid: String,
    pub name: String,
    // pub visibility: String,
    // pub created_at: String,
    // Add other fields as needed
}
#[derive(Debug, FromRow, Serialize)]
pub struct ListUser {
    pub uuid: String,
    pub pseudo: String,
    // pub visibility: String,
    // pub created_at: String,
    // Add other fields as needed
}

fn my_custom_date_serializer<S>(
    date: &NaiveDateTime,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let formatted = date.format("%d/%m/%Y %H:%M:%S").to_string();
    serializer.serialize_str(&formatted)
}

pub async fn get_room_with_messages(
    pool: &SqlitePool,
    room_uuid: &str,
) -> Result<(Room, Vec<MessageWithUser>), sqlx::Error> {
    // println!("get_room_with_messages demarrage du loading");
    println!("Chargement des messages pour room_uuid = {}", room_uuid);

    let room_result = sqlx::query_as::<_, Room>(
        "SELECT room_uuid, name, owner_uuid FROM rooms WHERE room_uuid = ? LIMIT 1")
    .bind(room_uuid)
    .fetch_one(pool)
    .await;

    let room = match room_result {
        Ok(r) => r,
        Err(e) => {
            println!("Erreur lors de la récupération de la room: {:?}", e);
            return Err(e);
        }
    };

    // let messages_result = sqlx::query_as::<_, Message>(
    //     "SELECT id, user_uuid, room_uuid, content, created_at FROM messages WHERE room_uuid = ? ORDER BY created_at ASC")
    //     .bind(room_uuid)
    //     .fetch_all(pool)
    //     .await;
    let messages_result = sqlx::query_as::<_, MessageWithUser>(
        "SELECT 
            m.id, 
            m.room_uuid, 
            m.user_uuid, 
            u.pseudo as user_name,
            m.content, 
            m.created_at
         FROM messages m
         JOIN users u ON m.user_uuid = u.uuid
         WHERE m.room_uuid = ?
         ORDER BY m.created_at ASC")
        .bind(room_uuid)
        .fetch_all(pool)
        .await;
    
    let messages = match messages_result {
        Ok(m) => {
            println!("########## message: {:?}",m);
            m
        },
        Err(e) => {
            println!("Erreur lors de la récupération des messages: {:?}", e);
            return Err(e);
        }
    };
    // println!("Messages reçu par le serveur\n {:?}", messages);
    Ok((room, messages))
}

// Verifie si l'utilisateur appartient à la room
pub async fn is_user_in_room(pool: &sqlx::SqlitePool, room_uuid: &str, user_uuid: &str) -> Result<bool, sqlx::Error> {
    println!("is_user_in_room ?");
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM room_members WHERE room_uuid = ? AND user_uuid = ?"
    )
    .bind(room_uuid)
    .bind(user_uuid)
    .fetch_one(pool)
    .await?;
    // if let Err(e) = count {
    //     println!(
    //         "Erreur lors de l'insertion du message pour {}: {:?}",
    //         room_uuid, e
    //     );
    // }

    println!("============> true or false: {}", count.0);
    Ok(count.0 > 0)
}

// ajoute un message dans une room pour un utilisateur
pub async fn insert_message(
    pool: &SqlitePool,
    room_uuid: &str,
    user_uuid: &str,
    content: &str,
) -> Result<(), sqlx::Error> {
    println!("insert_message ? room_uuid:{} user_uuid{}, \ncontent: {}",room_uuid, user_uuid, content);
    let result = sqlx::query(
        "INSERT INTO messages (room_uuid, user_uuid, content) VALUES (?, ?, ?)"
    )
    .bind(room_uuid)
    .bind(user_uuid)
    .bind(content)
    .execute(pool)
    .await; //?
    if let Err(e) = result {
        println!(
            "Erreur lors de l'insertion du message pour {}: {:?}",
            room_uuid, e
        );
    }

    Ok(())
}


//Recupère le nom de l'utilisateur grace à son uuid
pub async fn get_user_name(pool: &sqlx::SqlitePool, user_uuid: &str) -> Result<String, sqlx::Error>  {
    println!("is_user_in_room ?");
    let pseudo: (String,) = sqlx::query_as(
        "SELECT pseudo FROM users WHERE uuid = ?"
    )
    .bind(user_uuid)
    .fetch_one(pool)
    .await?;
    // if let Err(e) = count {
    //     println!(
    //         "Erreur lors de l'insertion du message pour {}: {:?}",
    //         room_uuid, e
    //     );
    // }

    println!("============> pseudo true or false: {}", pseudo.0);
    Ok(pseudo.0)
}

// Function to get all active rooms from database
pub async fn get_all_rooms(pool: &SqlitePool) -> Result<Vec<RoomSpe>, sqlx::Error> {
    let rooms = sqlx::query_as::<_, RoomSpe>("SELECT room_uuid, name FROM rooms")
        .fetch_all(pool)
        .await?;
    Ok(rooms)
}
pub async fn get_list_users(pool: &SqlitePool, pseudo: String) -> Result<Vec<ListUser>, sqlx::Error> {
    let list_users = sqlx::query_as::<_, ListUser>("SELECT uuid, pseudo FROM users Where pseudo != ?")
        .bind(pseudo)
        .fetch_all(pool)
        .await?;
    Ok(list_users)
}


/// Récupère tous les UUIDs des utilisateurs associés à une room donnée
pub async fn get_room_user_uuids(
    pool: &SqlitePool,
    room_uuid: &str,
) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query("SELECT user_uuid FROM room_members WHERE room_uuid = ?")
        .bind(room_uuid)
        .fetch_all(pool)
        .await?;

    // for row in rows {
    //     let uuid: String = row.get("user_uuid");
    //     let role: String = row.get("role");
    //     println!("User: {}, Role: {}", uuid, role);
    // }
    let user_uuids = rows
        .into_iter()
        .map(|row| row.get::<String, _>("user_uuid"))
        .collect();

    println!("\n#### voici les membre qui vont recevoir la room: {:?}", user_uuids);

    Ok(user_uuids)
}

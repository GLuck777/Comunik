use sqlx::SqlitePool;
use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use uuid::Uuid;
use tokio::sync::broadcast;
use crate::ws::ServerMessage;
use crate::sql_req::get_all_rooms;

// Initialize room broadcast channels from database
pub async fn initialize_rooms(
    pool: &SqlitePool, 
    rooms: &Arc<Mutex<HashMap<Uuid, broadcast::Sender<ServerMessage>>>>
) -> Result<(), sqlx::Error> {
    // Fetch all active rooms from database
    let db_rooms = get_all_rooms(pool).await?;
    
    // Create broadcast channel for each room
    let mut rooms_map = rooms.lock().unwrap();
    
    for room in db_rooms {
        if let Ok(room_uuid) = Uuid::parse_str(&room.room_uuid) {
            let (tx, _) = broadcast::channel(100); // 100 message capacity
            rooms_map.insert(room_uuid, tx);
            // println!("Initialized broadcast channel for room: {} ({})", room.name, room.room_uuid);
        } else {
            // eprintln!("Invalid room UUID format in database: {}", room.room_uuid);
        }
    }
    
    println!("Initialized {} room broadcast channels", rooms_map.len());
    Ok(())
}
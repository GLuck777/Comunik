// Dans un nouveau fichier: src/connection_manager.rs

use actix::Addr;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;
use tokio::sync::Mutex;

use crate::ws::{MyWebSocket, ServerMessage, DirectMessage};

/// Gestionnaire centralisé des connexions WebSocket
#[derive (Default, Debug)]
// Structure ConnectionManager améliorée
pub struct ConnectionManager {
    // Mapping UUID d'utilisateur -> Addr de WebSocket
    pub user_connections: HashMap<String, Addr<MyWebSocket>>, //alias sa connection à la websocket
    // Mapping UUID de room -> canal broadcast
    pub room_channels: HashMap<Uuid, broadcast::Sender<ServerMessage>>,
    // Données additionnelles sur les utilisateurs (optionnel)
    pub user_data: HashMap<String, UserData>,
}

/// Structure pour stocker des informations supplémentaires sur un utilisateur
#[derive(Clone, Debug)]
pub struct UserInfo {
    pub pseudo: String,
    pub email: Option<String>,
    pub last_seen: std::time::SystemTime,
    pub current_room: Option<String>,
}

#[derive(Debug)]
// Structure pour stocker des données utilisateur additionnelles
pub struct UserData {
    pub email: String,
    pub pseudo: String,
    pub last_activity: chrono::DateTime<chrono::Utc>,
    // Autres champs selon vos besoins
}

impl ConnectionManager {
    pub fn new() -> Self {
        Self {
            user_connections: HashMap::new(),
            room_channels: HashMap::new(),
            user_data: HashMap::new(),
        }
    }
    
    // Enregistrer un utilisateur avec ses données
    pub fn register_connection(&mut self, user_uuid: String, addr: Addr<MyWebSocket>, user_data: Option<UserData>) {
        self.user_connections.insert(user_uuid.clone(), addr);
        
        // Si des données utilisateur sont fournies, les enregistrer aussi
        if let Some(data) = user_data {
            self.user_data.insert(user_uuid, data);
        }
    }
    
    // Désenregistrer un utilisateur
    pub fn unregister_connection(&mut self, user_uuid: &str) {
        self.user_connections.remove(user_uuid);
        self.user_data.remove(user_uuid);
    }
    
    // Envoyer un message direct à un utilisateur
    pub fn send_direct_message(&self, target_user: &str, message: DirectMessage) -> bool {
        if let Some(addr) = self.user_connections.get(target_user) {
            addr.do_send(message);
            true
        } else {
            false
        }
    }
    
    // Envoyer un message à plusieurs utilisateurs
    pub fn broadcast_to_users(&self, target_users: &[String], message: DirectMessage) -> Vec<String> {
        let mut failed_users = Vec::new();
        
        for user_uuid in target_users {
            let success = self.send_direct_message(user_uuid, message.clone());
            if !success {
                failed_users.push(user_uuid.clone());
            }
        }
        
        failed_users
    }
    
    // Vérifier si un utilisateur est connecté
    pub fn is_user_connected(&self, user_uuid: &str) -> bool {
        self.user_connections.contains_key(user_uuid)
    }
    
    // Obtenir la liste des utilisateurs connectés
    pub fn get_connected_users(&self) -> Vec<String> {
        self.user_connections.keys().cloned().collect()
    }
    
    // Obtenir les données d'un utilisateur
    pub fn get_user_data(&self, user_uuid: &str) -> Option<&UserData> {
        self.user_data.get(user_uuid)
    }
}

// impl ConnectionManager {
//     /// Crée une nouvelle instance du gestionnaire de connexions
//     pub fn new() -> Self {
//         Self {
//             user_connections: HashMap::new(),
//             room_channels: HashMap::new(),
//             user_status: HashMap::new(),
//             user_info: HashMap::new(),
//         }
//     }
    
//     /// Enregistre une nouvelle connexion WebSocket
//     pub fn register_connection(&mut self, user_uuid: String, addr: Addr<MyWebSocket>) {
//         println!("Enregistrement de la connexion WebSocket pour {}", user_uuid);
        
//         // Stocker l'adresse de la connexion
//         self.user_connections.insert(user_uuid.clone(), addr);
        
//         // Mettre à jour le statut de connexion
//         self.user_status.insert(user_uuid.clone(), true);
        
//         // Mettre à jour la date de dernière connexion
//         if let Some(info) = self.user_info.get_mut(&user_uuid) {
//             info.last_seen = std::time::SystemTime::now();
//         }
//     }
    
//     /// Désenregistre une connexion WebSocket
//     pub fn unregister_connection(&mut self, user_uuid: &str) {
//         println!("Désenregistrement de la connexion WebSocket pour {}", user_uuid);
        
//         // Supprimer l'adresse de la connexion
//         self.user_connections.remove(user_uuid);
        
//         // Mettre à jour le statut de connexion
//         self.user_status.insert(user_uuid.to_string(), false);
        
//         // Mettre à jour la date de dernière connexion
//         if let Some(info) = self.user_info.get_mut(user_uuid) {
//             info.last_seen = std::time::SystemTime::now();
//             info.current_room = None;
//         }
//     }
    
//     /// Définit la room actuelle d'un utilisateur
//     pub fn set_user_room(&mut self, user_uuid: &str, room_uuid: Option<&str>) {
//         if let Some(info) = self.user_info.get_mut(user_uuid) {
//             info.current_room = room_uuid.map(|s| s.to_string());
//         }
//     }
    
//     /// Ajoute ou met à jour les informations d'un utilisateur
//     pub fn update_user_info(&mut self, user_uuid: &str, pseudo: &str, email: Option<&str>) {
//         let now = std::time::SystemTime::now();
        
//         self.user_info.insert(user_uuid.to_string(), UserInfo {
//             pseudo: pseudo.to_string(),
//             email: email.map(|s| s.to_string()),
//             last_seen: now,
//             current_room: None,
//         });
//     }
    
//     /// Envoie un message direct à un utilisateur spécifique
//     /// Retourne true si le message a été envoyé, false sinon
//     pub fn send_direct_message(&self, target_user: &str, message: DirectMessage) -> bool {
//         if let Some(addr) = self.user_connections.get(target_user) {
//             addr.do_send(message);
//             true
//         } else {
//             println!("Utilisateur {} non connecté, impossible d'envoyer le message", target_user);
//             false
//         }
//     }
    
//     /// Envoie un message à tous les utilisateurs connectés
//     pub fn broadcast_to_all(&self, message: DirectMessage) {
//         for (user_uuid, addr) in &self.user_connections {
//             addr.do_send(message.clone());
//         }
//     }
    
//     /// Envoie un message à tous les utilisateurs dans une room spécifique
//     pub fn broadcast_to_room(&self, room_uuid: &str, message: ServerMessage) -> bool {
//         match Uuid::parse_str(room_uuid) {
//             Ok(room_id) => {
//                 if let Some(sender) = self.room_channels.get(&room_id) {
//                     let _ = sender.send(message);
//                     true
//                 } else {
//                     println!("Room {} non trouvée", room_uuid);
//                     false
//                 }
//             },
//             Err(_) => {
//                 println!("UUID de room invalide: {}", room_uuid);
//                 false
//             }
//         }
//     }
    
//     /// Vérifie si un utilisateur est connecté
//     pub fn is_user_connected(&self, user_uuid: &str) -> bool {
//         self.user_connections.contains_key(user_uuid)
//     }
    
//     /// Récupère la liste des utilisateurs connectés
//     pub fn get_connected_users(&self) -> Vec<String> {
//         self.user_connections.keys().cloned().collect()
//     }
    
//     /// Récupère le nombre d'utilisateurs connectés
//     pub fn get_connected_users_count(&self) -> usize {
//         self.user_connections.len()
//     }
// }

// Créer une instance globale accessible de partout
// lazy_static::lazy_static! {
//     pub static ref GLOBAL_CONNECTION_MANAGER: Arc<Mutex<ConnectionManager>> = Arc::new(Mutex::new(ConnectionManager::new()));
// }
// Créez une instance globale de ConnectionManager
lazy_static::lazy_static! {
    pub static ref CONNECTION_MANAGER: Arc<Mutex<ConnectionManager>> = Arc::new(Mutex::new(ConnectionManager::new()));
}
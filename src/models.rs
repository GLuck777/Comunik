use serde::{Deserialize, Serialize};

#[derive(Debug, sqlx::FromRow)]
pub struct User {
    pub id: i64,
    pub uuid: String,
    pub email: String,
    pub password: String,
    pub pseudo: String,
}
use serde::Deserialize;
use crate::models::user::UserRole;
use sqlx::FromRow;

#[derive(Deserialize, Debug, Clone)]
pub struct LoginUser {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize, Debug, Clone, FromRow)]
pub struct LoginUserFromDatabase {
    pub id: String,
    pub email: String,
    pub password: String,
    pub role: UserRole,
}

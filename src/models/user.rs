use serde::{Deserialize, Serialize};
use uuid::Uuid;
use time::OffsetDateTime;
use sqlx::FromRow;

#[derive(Serialize, Deserialize, Debug, Clone, FromRow)]
pub struct User {
   pub id: Uuid,
   pub name: String,
   pub email: String,
   pub created_at: OffsetDateTime,
   pub password: String,
}

#[derive( Deserialize, Debug, Clone)]
pub struct CreateUser {
    pub name: String,
    pub email: String,
    pub password: String,
}

impl User {
    pub fn new(cre: &CreateUser, hash_password: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: cre.name.clone(),
            email: cre.email.clone(),
            created_at: OffsetDateTime::now_utc(),
            password: hash_password,
        }
    }

}

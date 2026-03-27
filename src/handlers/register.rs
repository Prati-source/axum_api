use crate::models::user::{CreateUser, User};
use argon2::{password_hash::{SaltString, rand_core::OsRng}, Argon2, PasswordHasher};
use sqlx::{PgPool,self};
use axum::{extract::State,http::StatusCode, Json};


pub async fn register_handler(State(pool): State<PgPool>, Json(cre): Json<CreateUser>) -> Result<(StatusCode, Json<User>),StatusCode > {
    if cre.email.is_empty() || cre.name.is_empty() {
        return  Err(StatusCode::BAD_REQUEST);
    }
    let argon2= Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = argon2.hash_password(cre.password.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string();
    let user = User::new(&cre , hashed_password);
    let x: User = sqlx::query_as!(User, "INSERT INTO users (id, name, email, created_at, password)
        VALUES ($1, $2, $3, $4, $5) RETURNING *",
        user.id, user.name, user.email, user.created_at, user.password)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert user");



    Ok((StatusCode::CREATED, Json(x)))


}

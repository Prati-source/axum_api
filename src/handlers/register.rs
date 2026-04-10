use crate::models::user::{CreateUser, User};
use argon2::{
    password_hash::{rand_core::OsRng, SaltString},
    Argon2, PasswordHasher,
};
use axum::{extract::State, http::StatusCode, Json};
use sqlx::{self, PgPool};

pub async fn register_handler(
    State(pool): State<PgPool>,
    Json(cre): Json<CreateUser>,
) -> Result<(StatusCode, Json<User>), StatusCode> {
    if cre.email.is_empty() || cre.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let existing_user: Option<User> =
        sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = $1")
            .bind(&cre.email)
            .fetch_optional(&pool)
            .await
            .expect("Failed to query user");
    if existing_user.is_some() {
        return Err(StatusCode::CONFLICT);
    }

    let argon2 = Argon2::default();
    let salt = SaltString::generate(&mut OsRng);
    let hashed_password = argon2
        .hash_password(cre.password.as_bytes(), &salt)
        .expect("Failed to hash password")
        .to_string();
    let user = User::new(&cre, hashed_password);
    let user_out = user.clone();
    sqlx::query(
        "INSERT INTO users (id, name, email, created_at, password)
        VALUES ($1, $2, $3, $4, $5) RETURNING *",
    )
    .bind(user.id)
    .bind(user.name)
    .bind(user.email)
    .bind(user.created_at)
    .bind(user.password)
    .fetch_one(&pool)
    .await
    .expect("Failed to insert user");

    Ok((StatusCode::CREATED, Json(user_out)))
}

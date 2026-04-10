use crate::models::{login_user::LoginUser, user::User};
use argon2::{Argon2, PasswordVerifier};
use axum::{
    extract::State, http::header::SET_COOKIE, http::StatusCode, response::IntoResponse, Json,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde_json::json;
use sqlx::{self, PgPool};
use time::{Duration, OffsetDateTime};

fn create_token(user: User) -> Result<impl IntoResponse, (StatusCode, String)> {
    let claims = json!({
        "sub": user.id,
        "exp": OffsetDateTime::now_utc() + Duration::seconds(86400)
    });
    let secret = std::env::var("JWT_SECRET")
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let cookie = format!("token={}; HttpOnly; Secure; SameSite=Strict; Path=/", token);
    Ok(([(SET_COOKIE, cookie)], "Authenticated").into_response())
}

pub async fn login_handler(
    State(pool): State<PgPool>,
    Json(login_user): Json<LoginUser>,
) -> impl IntoResponse {
    if login_user.email.is_empty() || login_user.password.is_empty() {
        return (StatusCode::BAD_REQUEST, "Bad Request").into_response();
    }

    let user = sqlx::query_as::<_, User>(
        "SELECT id, name, email, created_at, password FROM users WHERE email = $1",
    )
    .bind(login_user.email)
    .fetch_one(&pool) // Use fetch_optional if the user might not exist
    .await;

    println!("{:?}", user);
    match user {
        Ok(user) => {
            let parsed_password = argon2::password_hash::PasswordHash::new(&user.password)
                .expect("Failed to parse password hash");
            println!("{:?}", parsed_password);
            match Argon2::default()
                .verify_password(login_user.password.as_bytes(), &parsed_password)
            {
                Ok(_) => {
                     match create_token(user) {
                        Ok(response) => response.into_response(),
                        Err((status, message)) => (status, message).into_response(),
                    }
                }
                Err(_) =>  (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
            }
        }
        Err(_) =>  (StatusCode::UNAUTHORIZED, "Unauthorized").into_response(),
    }
}

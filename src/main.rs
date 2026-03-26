use axum::{
    routing::{get, post},
    Json, Router,
};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod middlewares;
use middlewares::auth::auth_middleware;
use axum::middleware;
use sqlx::PgPool;
use dotenvy::dotenv;


#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

#[derive(Deserialize)]
struct CreateUser {
    name: String,
}

#[derive(Serialize)]
struct User {
    id: u32,
    name: String,
}

// Health check route
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

// GET /users
async fn get_users() -> Json<Vec<User>> {
    let users = vec![
        User { id: 1, name: "Alice".into() },
        User { id: 2, name: "Bob".into() },
    ];

    Json(users)
}

// POST /users
async fn create_user(Json(payload): Json<CreateUser>) -> Json<User> {
    let user = User {
        id: 3,
        name: payload.name,
    };

    Json(user)
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in .env");

        // create the connection pool
    let pool = PgPool::connect(&database_url)
            .await
            .expect("failed to connect to database");
    // run migrations automatically on startup
     sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .expect("migrations failed");

  tracing_subscriber::registry()
    .with(tracing_subscriber::filter::EnvFilter::new("tower_http=debug,axum=debug,info"))
    .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("App started");


let app = Router::new()
    .route("/health", get(health))
    .route("/users", get(get_users))
    .route("/users",post(create_user))
    .layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(middleware::from_fn(auth_middleware))

    )
    .with_state(pool)
   ;


    let addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    println!("Server running on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

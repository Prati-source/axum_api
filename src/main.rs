use axum::{
    routing::{get, post},
    Json, Router,
};
mod models;
use serde::Serialize;
use std::{net::SocketAddr, sync::Arc};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
mod bus;
mod handlers;
mod middlewares;
use axum::middleware;
use dotenvy::dotenv;
use handlers::{login::login_handler, register::register_handler, ws::ws_handler};
use middlewares::auth::auth_middleware;
use models::redis_state::AppState;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}
// Health check route
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}

// GET /users

#[tokio::main]
async fn main() {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");

    // create the connection pool
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");
    // run migrations automatically on startup
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations failed");
    //run redis connection
    let redis = redis::Client::open("redis://127.0.0.1:6379/").expect("failed to connect to redis");

    let state = Arc::new(AppState::new(redis).await);

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::EnvFilter::new(
            "tower_http=debug,axum=debug,info",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("App started");
    let location_router = Router::new()
        .route("/ws", get(ws_handler))
        .with_state(state);

    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .nest("/location", location_router)
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn(auth_middleware)),
        )
        .with_state(pool);

    let addr = SocketAddr::from((
        std::env::var("HOST")
            .unwrap_or("127.0.0.1".to_string())
            .parse::<std::net::IpAddr>()
            .unwrap(),
        std::env::var("PORT").unwrap().parse::<u16>().unwrap(),
    ));
    println!("Server running on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

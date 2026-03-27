use axum::{
    routing::{get, post},
    Json, Router,
};
mod models;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use serde::{ Serialize};
use std::net::SocketAddr;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
mod handlers;
mod middlewares;
use middlewares::auth::auth_middleware;
use axum::middleware;
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use dotenvy::dotenv;
use handlers::register::register_handler;


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
    let database_url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set in .env");

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

  tracing_subscriber::registry()
    .with(tracing_subscriber::filter::EnvFilter::new("tower_http=debug,axum=debug,info"))
    .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("App started");


let app = Router::new()
    .route("/health", get(health))
    .route("/register", post(register_handler))
    .layer(
        ServiceBuilder::new()
            .layer(TraceLayer::new_for_http())
            .layer(middleware::from_fn(auth_middleware))
    )
    .with_state(pool)
   ;


    let addr = SocketAddr::from((std::env::var("HOST").unwrap_or("127.0.0.1".to_string()).parse::<std::net::IpAddr>().unwrap()
        , std::env::var("PORT").unwrap().parse::<u16>().unwrap()));
    println!("Server running on {}", addr);

    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}

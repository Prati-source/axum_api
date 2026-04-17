use axum::{
    routing::{get, post},
    Json, Router,
};
mod models;
use serde::Serialize;
use std::{net::SocketAddr, sync::{Arc, OnceLock}};
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
mod bus;
mod handlers;
mod middlewares;
use axum::middleware;
use dotenvy::dotenv;
use handlers::{login::login_handler, register::register_handler, ws::ws_handler, customer::customer_handler, verify::verify_handler};
use middlewares::auth::auth_middleware;
use models::state::AppState;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use bus::redis_bus;
use redis::aio::ConnectionManager;
use lettre::{transport::smtp::authentication::Credentials, AsyncSmtpTransport, AsyncTransport, Tokio1Executor};
use axum_prometheus::PrometheusMetricLayer;



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

static WORKER_ID: OnceLock<String> = OnceLock::new();

// GET /users

#[tokio::main]
async fn main() {
    dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set in .env");
    let smtp_username = std::env::var("SMTP_USERNAME").expect("SMTP_USERNAME must be set in .env");
    let smtp_password = std::env::var("SMTP_PASSWORD").expect("SMTP_PASSWORD must be set in .env");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL must be set in .env");
    let id = format!("worker-{}", uuid::Uuid::new_v4());
    WORKER_ID.set(id).expect("failed to set worker id");
    // create the connection pool
    let pool: PgPool = PgPoolOptions::new()
        .max_connections(1000)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");
    // run migrations automatically on startup
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("migrations failed");
    //run redis connection
    let redis_client = redis::Client::open(redis_url).expect("failed to connect to redis");
    let redis_manager: ConnectionManager = ConnectionManager::new(redis_client.clone()).await.expect("failed to get redis connection");
    //SMTP credentials Connection
    let creds = Credentials::new(
        smtp_username.to_string(),
        smtp_password.to_string(),
    );
    let mailer = AsyncSmtpTransport::<Tokio1Executor>::starttls_relay("smtp.gmail.com")
        .unwrap()
        .credentials(creds)
        .port(587)
        .build();
    let (prometheus_layer, metric_handler) = PrometheusMetricLayer::pair();
    //running the all connection through Appstate
    let state = Arc::new(AppState::new(redis_manager, redis_client, pool, mailer).await);
    let background_state = Arc::clone(&state);

    tracing_subscriber::registry()
        .with(tracing_subscriber::filter::EnvFilter::new(
            "tower_http=debug,axum=debug,info",
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
    tracing::info!("App started");

    tokio::spawn(async move { //use tokio::spawn to run the background task of moving from redis stream to postgres
            // Using a 5-minute interval
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let _ = redis_bus::redis_stream_to_postgres(&background_state).await;
            }
    });


    let app = Router::new()
        .route("/health", get(health))
        .route("/register", post(register_handler))
        .route("/login", post(login_handler))
        .route("/ws", get(ws_handler))
        .route("/customer", get(customer_handler))
        .route("/verify", post(verify_handler))
        .route("/metrics", get(move || async move { metric_handler.render() }))
        .layer(
            ServiceBuilder::new()
                .layer(prometheus_layer)
                .layer(TraceLayer::new_for_http())
                .layer(middleware::from_fn(auth_middleware)),
        )
        .with_state(state);


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

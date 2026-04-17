use dashmap::DashMap;
use redis::{aio::ConnectionManager, Client};
use tokio::sync::broadcast;
use sqlx::PgPool;
use lettre::{AsyncSmtpTransport, Tokio1Executor};


/// One in-process broadcast sender per parcel.
/// Customers subscribe to this; the Redis listener feeds it.
#[derive(Clone)]
pub struct AppState {
    pub redis_manager : ConnectionManager,
    pub redis_client: Client,
    pub mailer: AsyncSmtpTransport<Tokio1Executor>,

    /// parcel_id → sender for that parcel's location stream
    pub pool: PgPool,
    pub parcels: DashMap<String, broadcast::Sender<String>>,
}

impl AppState {
    pub async fn new(redis_manager: ConnectionManager, redis_client: redis::Client, pool: PgPool, mailer: AsyncSmtpTransport<Tokio1Executor>) -> Self {
        Self {
            redis_manager,
            redis_client,
            mailer,
            pool,
            parcels: DashMap::new(),
        }
    }

    /// Get or create the in-process channel for a parcel.
    pub fn channel_for(&self, parcel_id: &str) -> broadcast::Sender<String> {
        self.parcels
            .entry(parcel_id.to_string())
            .or_insert_with(|| broadcast::channel::<String>(32).0)
            .clone()
    }
}

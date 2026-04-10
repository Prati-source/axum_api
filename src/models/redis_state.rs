use dashmap::DashMap;
use redis::Client;
use tokio::sync::broadcast;

/// One in-process broadcast sender per parcel.
/// Customers subscribe to this; the Redis listener feeds it.
#[derive(Clone)]
pub struct AppState {
    pub redis: Client,
    /// parcel_id → sender for that parcel's location stream
    pub parcels: DashMap<String, broadcast::Sender<String>>,
}

impl AppState {
    pub async fn new(redis: Client) -> Self {
        Self {
            redis,
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

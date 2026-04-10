use crate::AppState;
use futures::stream::StreamExt;
use redis::AsyncCommands;
use std::sync::Arc;

/// Channel name convention: one channel per parcel
fn channel(parcel_id: &str) -> String {
    format!("parcel:{parcel_id}")
}

/// Redis key for last known position hash
fn position_key(parcel_id: &str) -> String {
    format!("parcel:{parcel_id}")
}

/// Publish a location update to Redis.
/// Also persists it as the last known position.
pub async fn publish(
    state: &Arc<AppState>,
    parcel_id: &str,
    payload: &str,
) -> redis::RedisResult<()> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;

    // Broadcast to all subscribers on this channel
    let x: i32 = conn
        .publish::<_, _, i32>(channel(parcel_id), payload)
        .await?;
    if x == 1 {
        println!("New field created!");
    } else {
        println!("Existing field updated.");
    }
    // Persist last known position (customer joining late gets this immediately)
    conn.hset::<_, _, _, ()>(position_key(parcel_id), "data", payload)
        .await?;

    // Expire after 6 hours — parcel delivered, no need to keep forever
    conn.expire::<_, ()>(position_key(parcel_id), 6 * 3600)
        .await?;

    Ok(())
}

/// Fetch last known position for a parcel (for customers connecting mid-delivery).
pub async fn last_position(
    state: &Arc<AppState>,
    parcel_id: &str,
) -> redis::RedisResult<Option<String>> {
    let mut conn = state.redis.get_multiplexed_async_connection().await?;
    let temp = conn.hget(position_key(parcel_id), "data").await?;
    tracing::info!("{parcel_id}: last position: {:?}", temp);
    Ok(temp)
}

/// Subscribe to a parcel channel and fan-out into the in-process broadcast.
/// Spawned once per parcel when the first customer connects.
pub async fn subscribe_parcel(state: Arc<AppState>, parcel_id: String) {
    let mut pubsub = match state.redis.get_async_pubsub().await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("Redis pubsub connect failed: {e}");
            return;
        }
    };

    if let Err(e) = pubsub.subscribe(channel(&parcel_id)).await {
        tracing::error!("Redis subscribe failed: {e}");
        return;
    }

    tracing::info!("Redis subscriber started for parcel {parcel_id}");

    let mut stream = pubsub.on_message();
    loop {
        match stream.next().await {
            Some(msg) => {
                let payload: String = match msg.get_payload() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let tx = state.channel_for(&parcel_id);
                // No active WebSocket customers — clean up and stop
                if tx.receiver_count() == 0 {
                    state.parcels.remove(&parcel_id);
                    tracing::info!("No customers left for {parcel_id}, stopping subscriber");
                    break;
                }
                let _ = tx.send(payload);
            }
            None => break, // connection closed
        }
    }
}

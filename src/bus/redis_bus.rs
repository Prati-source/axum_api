use futures::stream::StreamExt;
use redis::{AsyncCommands, pipe, RedisResult, streams::{StreamRangeReply, StreamReadOptions, StreamReadReply}, Client};
use std::sync::Arc;
use crate::WORKER_ID;
use crate::models::error::SyncError;
use crate::models::{user::User, state::AppState};
/// Channel name convention: one channel per parcel
fn channel(parcel_id: &str) -> String {
    format!("parcel:{parcel_id}")
}

/// Redis key for last known position hash
fn position_key(parcel_id: &str) -> String {
    format!("parcel:{parcel_id}")
}

fn geo_key() -> String {
    format!("active_drivers")
}

fn history_key() -> String {
    format!("parcel:history")
}

/// Publish a location update to Redis.
/// Also persists it as the last known position.
pub async fn publish(
    state: &Arc<AppState>,
    parcel_id: &str,
    payload: &str,
    lat: &f64,
    lon: &f64,
    driver_id: &str,
) -> Result<(), SyncError> {
    let mut conn = state.redis_manager.clone();

    // Broadcast to all subscribers on this channel
    let _: () = pipe()
        .publish(channel(parcel_id), payload)
        .geo_add(geo_key(), (lat, lon, driver_id))
        .query_async(&mut conn)
        .await?;
    // Persist last known position (customer joining late gets this immediately)
    let x = conn.hset::<_, _, _, ()>(position_key(parcel_id), "data", payload)
        .await?;
    tracing::info!("{:?}", x);

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
    let mut conn = state.redis_manager.clone();
    let temp = conn.hget(position_key(parcel_id), "data").await?;
    tracing::info!("{parcel_id}: last position: {:?}", temp);
    Ok(temp)
}

/// Subscribe to a parcel channel and fan-out into the in-process broadcast.
/// Spawned once per parcel when the first customer connects.
pub async fn subscribe_parcel(parcel_id: String, state: Arc<AppState>) {
    let state_red = state.redis_client.clone();
    let mut pubsub = match state_red.get_async_pubsub().await {
           Ok(c) => c,
           Err(e) => {
               eprintln!("Redis connection error: {}", e);
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

/// Publish a message to the Redis stream for the given parcel after a delay
pub async fn redis_stream_publish(state: &Arc<AppState>, parcel_id: &str) -> RedisResult<()> {
    let mut stream = state.redis_manager.clone();
    let (payload, last_position_stream): (Option<String>, StreamRangeReply) = pipe()
        .hget(position_key(parcel_id), "data")
        .xrevrange_count(history_key(), "+", "-", 1)
        .query_async(&mut stream)
        .await?;
    tracing::info!("Publishing stream for {parcel_id}, last position: {:?}, payload_of_last_position: {:?}", last_position_stream.ids.first(), payload);
    if last_position_stream.ids.first().is_none() {
        let _x:() = stream.xadd(history_key(), "*", &[("payload", &payload)]).await?;
        return Ok(());
    }
    if let Some(entry) = last_position_stream.ids.first() {
        let last_payload: String = entry.get::<String>("payload").unwrap_or_default().to_string();
        tracing::info!("Last payload: {last_payload}");
        if payload.as_deref() == Some(&last_payload) {
            tracing::info!("Duplicate payload for {:?} and {last_payload}, skipping", payload);
            return Ok(());
        }
        let _x:() = stream.xadd(history_key(), "*", &[("payload", &payload)]).await?;
        return Ok(());
    }

    Ok(())
}

/// Send batch message from Redis stream to the postgres history table
pub async fn redis_stream_to_postgres(state: &Arc<AppState>) -> Result<(), SyncError> {
    let worker_id = WORKER_ID.get().expect("worker id not set");
    let opts = StreamReadOptions::default()
        .group("history", &worker_id)
        .count(1000)
        .block(5000);
    let mut stream = state.redis_manager.clone();
    let stream_response: StreamReadReply = stream
        .xread_options( &[history_key()], &[">"], &opts)
        .await?;
    let mut parcel_ids = Vec::new();
    let mut driver_ids = Vec::new();
    let mut latitudes = Vec::new();
    let mut longitudes = Vec::new();
    let mut timestamps = Vec::new();
    let mut statuses = Vec::new();
    if stream_response.keys.is_empty() {
        tracing::warn!("no history entries found for in redis stream");
        return Ok(());
    }
    for stream_key in stream_response.keys {
        for entry in stream_key.ids {
            parcel_ids.push(entry.get::<String>("parcel_id").unwrap_or_default().to_string());
            driver_ids.push(entry.get::<String>("driver_id").unwrap_or_default().to_string());
            latitudes.push(entry.get::<f64>("latitude").unwrap_or_default() as f64);
            longitudes.push(entry.get::<f64>("longitude").unwrap_or_default() as f64);
            timestamps.push(entry.get::<u64>("timestamp").unwrap_or_default() as i64);
            statuses.push(entry.get::<String>("status").unwrap_or_default().to_string());
        }
    }
    if !parcel_ids.is_empty() {
        sqlx::query!(
            r#"
            INSERT INTO parcel_history (parcel_id, driver_id, latitude, longitude, timestamp, status)
             SELECT u.p_id, u.d_id, u.lat, u.lon, u.ts, u.status
             FROM UNNEST($1::text[], $2::text[], $3::float[], $4::float[], $5::bigint[], $6::text[]) AS u(p_id, d_id, lat, lon, ts, status)"#,
             &parcel_ids, // Rust knows this must be a Vec<String>
             &driver_ids,  // If types don't match the DB, it won't compile
             &latitudes,
             &longitudes,
             &timestamps,
             &statuses,
        )
        .execute(&state.pool)
        .await?;
    }

    Ok(())
}


pub async fn publish_otp(otp: &u32, user: &User, state: &Arc<AppState>) -> Result<(), SyncError> {
    let otp_str = otp.to_string();
    let user_string = serde_json::to_string(user).unwrap_or_default();
    let mut stream = state.redis_manager.clone();
    let _: ((), ()) = pipe()
        .set_ex(format!("otp for {}", user.email), otp_str, 300)
        .set_ex(format!("pending for {}", user.email), user_string, 900)
        .query_async(&mut stream)
        .await?;
    Ok(())
}

pub async fn read_otp(otp: &u32, email: &str, state: &Arc<AppState>) -> Result<(), SyncError> {
    let mut stream = state.redis_manager.clone();
    let otp_str: Option<String> = stream.get(format!("otp for {}", email)).await?;
    let otp_user: Option<u32> = otp_str.map(|s| s.parse().ok()).flatten();
    println!("otp_user: {:?}, otp: {:?}", otp_user, otp);
    if otp_user == Some(*otp) {
        let user_str: Option<String> = stream.get(format!("pending for {}", email)).await?;
        let user: User = serde_json::from_str(&user_str.unwrap_or_default()).map_err(|e| SyncError::Json(e))?;
        let user_role: String = user.role.to_string();
        tracing::info!("OTP verified for email: {:?}", user);
        let _: ((), ()) = pipe()
            .del(format!("otp for {}", email))
            .del(format!("pending for {}", email))
            .query_async(&mut stream)
            .await?;
        let x = sqlx::query("INSERT INTO users (id, name, email, password, created_at, role AS text) VALUES ($1, $2, $3, $4, $5, $6)")
            .bind(&user.id)
            .bind(&user.name)
            .bind(&user.email)
            .bind(&user.password)
            .bind(&user.created_at)
            .bind(&user_role)
            .execute(&state.pool)
            .await?;
        tracing::info!("User inserted into database: {:?}", x);

    }
    Ok(())
}

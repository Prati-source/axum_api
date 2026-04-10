use crate::bus::redis_bus;
use crate::models::{
    location_user::{ConnectParams, LocationUpdate},
    redis_state::AppState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<ConnectParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| async move {
        match params.role.as_str() {
            "driver" => handle_driver(socket, state, params.parcel_id).await,
            "customer" => handle_customer(socket, state, params.parcel_id).await,
            _ => { /* unknown role — socket closes */ }
        }
    })
}

/// Driver side: receive location from socket, publish to Redis
async fn handle_driver(mut socket: WebSocket, state: Arc<AppState>, parcel_id: String) {
    tracing::info!("Driver connected for parcel {parcel_id}");

    while let Some(Ok(Message::Text(text))) = socket.recv().await {
        // Validate the shape before publishing
        tracing::debug!(
            "Received location update: {:?}",
            serde_json::from_str::<LocationUpdate>(&text)
        );
        if serde_json::from_str::<LocationUpdate>(&text).is_err() {
            tracing::warn!("Invalid location payload, dropping");
            continue;
        }
        if let Err(e) = redis_bus::publish(&state, &parcel_id, &text).await {
            tracing::error!("Redis publish error: {e}");
        }
    }

    tracing::info!("Driver disconnected for parcel {parcel_id}");
}

/// Customer side: subscribe to Redis channel, push to WebSocket
async fn handle_customer(mut socket: WebSocket, state: Arc<AppState>, parcel_id: String) {
    tracing::info!("Customer connected for parcel {parcel_id}");

    // Send last known position immediately so the map isn't blank
    if let Ok(Some(last)) = redis_bus::last_position(&state, &parcel_id).await {
        tracing::info!("{parcel_id}: last position: {last}");
        let _ = socket.send(Message::Text(last.into())).await;
    }

    // Ensure a Redis subscriber task is running for this parcel
    let tx = state.channel_for(&parcel_id);
    if tx.receiver_count() == 0 {
        // First customer — spawn the Redis subscriber
        tokio::spawn(redis_bus::subscribe_parcel(
            state.clone(),
            parcel_id.clone(),
        ));
    }
    let mut rx = tx.subscribe();

    loop {
        tokio::select! {
            // Forward Redis messages to this customer's WebSocket
            Ok(msg) = rx.recv() => {
                 tracing::info!("{parcel_id}: {msg}");
                if socket.send(Message::Text(msg.into())).await.is_err() {
                    break; // customer disconnected
                }

            }
            // Handle ping / close from customer side
            Some(Ok(msg)) = socket.recv() => {
                match msg {
                    Message::Close(_) => break,
                    Message::Ping(p) => { let _ = socket.send(Message::Pong(p)).await; }
                    _ => {}
                }
            }
            else => break,
        }
    }

    tracing::info!("Customer disconnected for parcel {parcel_id}");
}

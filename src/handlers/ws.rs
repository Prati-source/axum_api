use crate::bus::redis_bus;
use crate::models::{
    location_user::{ConnectParams, LocationUpdate},
    state::AppState,
};
use axum::{
    extract::{
        ws::{Message, WebSocket},
        Query, State, WebSocketUpgrade,
    },
    response::IntoResponse,
};
use std::sync::Arc;
use tokio::time::{ Duration, interval};
use reqwest::StatusCode;


pub async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<ConnectParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {

    ws.on_upgrade(move |socket| async move {
        match params.role.as_str() {
            "driver" => handle_driver(socket, state, params.parcel_id).await,
            _ => { /* unauthorized */ }
        }
    })
}

/// Driver side: receive location from socket, publish to Redis
async fn handle_driver(mut socket: WebSocket, state: Arc<AppState>, parcel_id: String) {
    tracing::info!("Driver connected for parcel {parcel_id}");
    let mut stream_tick = interval(Duration::from_secs(20));
    stream_tick.tick().await;
    loop {
        tokio::select! {
                msg = socket.recv() => {
                // Validate the shape before publishing
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(update) = serde_json::from_str::<LocationUpdate>(&text) {
                            if let Err(e) = redis_bus::publish(&state, &parcel_id, &text, &update.latitude, &update.longitude, &update.driver_id).await {
                                tracing::error!("Redis publish error: {e}");
                                break;
                            }
                            socket.send(Message::Text("Success".into())).await.ok();
                            continue;
                        } else {
                            tracing::warn!("Invalid location payload, dropping {text}");
                            socket.send(Message::Text("Close".into())).await.ok();
                            break;
                        }
                    },
                    Some(Err(e)) => { tracing::warn!("Invalid location payload , Error: {:?}", e); socket.send(Message::Text("Close".into())).await.ok(); break; }
                    Some(Ok(_)) => {  tracing::info!("Location update received for parcel {parcel_id}"); socket.send(Message::Text("Location not properly formatted".into())).await.ok(); break; }
                    None => { tracing::warn!("Invalid location payload, dropping nothing"); socket.send(Message::Text("Close".into())).await.ok(); break; }
                }//match msg

            }//msg
                _ = stream_tick.tick() => {
                    tracing::info!("Sending ping for parcel stream {parcel_id}");
                if let Err(e) = redis_bus::redis_stream_publish(&state, &parcel_id).await {
                    tracing::error!("Redis stream publish error: {e}");
                    socket.send(Message::Text("Close".into())).await.ok();
                } else {
                    socket.send(Message::Text("Stream".into())).await.ok();
                }
                continue;
            }


        }//tokio: select

    }//loop

    tracing::info!("Driver disconnected for parcel {parcel_id}");
}

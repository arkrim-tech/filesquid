//! Integration test: events published on the API bus reach a WebSocket client of
//! `GET /api/v1/events` on a real TCP listener.

use filesquid_api::auth::Authoriser;
use filesquid_api::{ApiEvent, ApiState};
use filesquid_server::{app, ServerConfig};
use futures_util::StreamExt;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::Request;
use tokio_tungstenite::tungstenite::Message;

/// Administrator token the test server accepts.
const TOKEN: &str = "events-token";

/// API state whose authoriser accepts [`TOKEN`].
fn authorised_state() -> ApiState {
    let state = ApiState::new();
    state.install_authoriser(Authoriser::server_administrator(TOKEN));
    state
}

/// WebSocket handshake request for the event channel carrying [`TOKEN`] as a bearer header.
fn events_request(addr: SocketAddr) -> Request<()> {
    let mut request = format!("ws://{addr}/api/v1/events")
        .into_client_request()
        .expect("request");
    let bearer = format!("Bearer {TOKEN}").parse().expect("header value");
    request.headers_mut().insert("authorization", bearer);
    request
}

#[tokio::test]
async fn websocket_clients_receive_published_events() {
    let state = authorised_state();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral port");
    let addr = listener.local_addr().expect("addr");
    let router = app(&ServerConfig::default(), state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await });

    let (mut socket, _) = tokio_tungstenite::connect_async(events_request(addr))
        .await
        .expect("websocket handshake");

    // Publish until the server-side subscriber is attached (bounded: 50 tries x 20 ms).
    let mut delivered = 0;
    for _ in 0..50 {
        delivered = state.events.publish(ApiEvent::new(
            "job.progress",
            serde_json::json!({ "done": 1 }),
        ));
        if delivered > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(delivered > 0, "server subscribed to the event bus");

    let frame = tokio::time::timeout(Duration::from_secs(5), socket.next())
        .await
        .expect("event within 5 s")
        .expect("stream open")
        .expect("frame");
    let Message::Text(text) = frame else {
        panic!("expected a text frame, got {frame:?}");
    };
    let event: ApiEvent = serde_json::from_str(&text).expect("ApiEvent JSON");
    assert_eq!(event.topic, "job.progress");
    server.abort();
}

#[tokio::test]
async fn closing_an_idle_client_drops_its_subscription() {
    let state = authorised_state();
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("ephemeral port");
    let addr = listener.local_addr().expect("addr");
    let router = app(&ServerConfig::default(), state.clone());
    let server = tokio::spawn(async move { axum::serve(listener, router).await });

    let (mut socket, _) = tokio_tungstenite::connect_async(events_request(addr))
        .await
        .expect("websocket handshake");
    let noop = || ApiEvent::new("noop", serde_json::Value::Null);

    // Wait until the server-side subscriber is attached (bounded: 50 tries x 20 ms).
    let mut attached = false;
    for _ in 0..50 {
        attached = state.events.publish(noop()) > 0;
        if attached {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(attached, "server subscribed to the event bus");

    socket.close(None).await.expect("client close");

    // With no further events, the subscription must still go away (bounded: 50 tries x 20 ms).
    let mut released = false;
    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        released = state.events.subscriber_count() == 0;
        if released {
            break;
        }
    }
    assert!(released, "idle disconnect released the bus subscription");
    server.abort();
}

use futures::StreamExt;
use tokio::net::UnixListener;
use tokio_tungstenite::tungstenite::Message;

use super::*;
use crate::config::ClientConfig;
use crate::transport::Client;

/// Spawns a fake Incus daemon that accepts one WebSocket connection on a
/// Unix socket and sends the given canned text frames before closing.
async fn spawn_fake_events_daemon(frames: Vec<String>) -> (std::path::PathBuf, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("create temp dir");
    let socket_path = dir.path().join("incus.sock");
    let listener = UnixListener::bind(&socket_path).expect("bind unix listener");

    tokio::spawn(async move {
        let Ok((stream, _)) = listener.accept().await else {
            return;
        };
        let mut ws = tokio_tungstenite::accept_async(stream)
            .await
            .expect("accept websocket handshake");
        for frame in frames {
            use futures::SinkExt;
            let _ = ws.send(Message::Text(frame.into())).await;
        }
        let _ = ws.close(None).await;
    });

    (socket_path, dir)
}

#[tokio::test]
async fn subscribe_events_yields_a_typed_operation_event() {
    let frame = r#"{"type":"operation","metadata":{"id":"11111111-1111-1111-1111-111111111111","class":"task","status":"Success","status_code":200,"resources":{},"may_cancel":false,"err":null}}"#.to_owned();
    let (socket_path, _dir) = spawn_fake_events_daemon(vec![frame]).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let mut stream = client
        .subscribe_events(EventFilter::default())
        .await
        .expect("subscribe should succeed");

    let event = stream
        .next()
        .await
        .expect("stream should yield one event")
        .expect("event should parse successfully");

    match event {
        Event::Operation(op) => assert_eq!(op.status_code, 200),
        other => panic!("expected Event::Operation, got {other:?}"),
    }
}

#[tokio::test]
async fn subscribe_events_yields_typed_lifecycle_and_logging_events() {
    let lifecycle = r#"{"type":"lifecycle","metadata":{"action":"instance-started","source":"/1.0/instances/c1"}}"#.to_owned();
    let logging = r#"{"type":"logging","metadata":{"message":"hello","level":"info"}}"#.to_owned();
    let (socket_path, _dir) = spawn_fake_events_daemon(vec![lifecycle, logging]).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let mut stream = client
        .subscribe_events(EventFilter::default())
        .await
        .expect("subscribe should succeed");

    let first = stream.next().await.unwrap().unwrap();
    assert!(matches!(first, Event::Lifecycle(_)));
    let second = stream.next().await.unwrap().unwrap();
    assert!(matches!(second, Event::Logging(_)));
}

#[test]
fn event_filter_default_subscribes_to_everything() {
    let filter = EventFilter::default();
    assert!(filter.operations);
    assert!(filter.lifecycle);
    assert!(filter.logging);
}

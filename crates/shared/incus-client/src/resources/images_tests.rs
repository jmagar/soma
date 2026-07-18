use super::*;
use crate::config::ClientConfig;
use crate::transport::{
    unix::tests::{json_response, spawn_fake_daemon},
    Client,
};

fn image_json(fingerprint: &str) -> String {
    format!(
        r#"{{"type":"sync","status":"Success","status_code":200,"metadata":{{"fingerprint":"{fingerprint}","public":false,"filename":"debian-12.tar.xz","size":123456,"architecture":"x86_64","created_at":"2026-01-01T00:00:00Z","uploaded_at":"2026-01-01T00:00:00Z","properties":{{}}}}}}"#
    )
}

fn operation_json(id: &str) -> String {
    format!(
        r#"{{"type":"async","status":"Operation created","status_code":100,"operation":"/1.0/operations/{id}","metadata":{{"id":"{id}","class":"task","status":"Running","status_code":103,"resources":{{}},"may_cancel":true,"err":null}}}}"#
    )
}

#[tokio::test]
async fn get_image_deserializes_the_documented_shape() {
    let body = image_json("abc123");
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 200 OK", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let image = client
        .get_image("abc123")
        .await
        .expect("get_image should succeed");
    assert_eq!(image.fingerprint, "abc123");
    assert_eq!(image.filename, "debian-12.tar.xz");
}

#[tokio::test]
async fn list_images_requires_explicit_recursion() {
    let seen_request = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let seen = seen_request.clone();
    let body = r#"{"type":"sync","status":"Success","status_code":200,"metadata":[]}"#;
    let (socket_path, _dir) = spawn_fake_daemon(move |req| {
        *seen.lock().unwrap() = String::from_utf8_lossy(&req).into_owned();
        json_response("HTTP/1.1 200 OK", body)
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    client
        .list_images(false)
        .await
        .expect("list_images should succeed");
    assert!(seen_request.lock().unwrap().contains("recursion=false"));
}

#[tokio::test]
async fn create_image_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .create_image(&serde_json::json!({"source": {"type": "url", "url": "https://example.com/image.tar.xz"}}))
        .await
        .expect("create_image should return an Operation - image creation is documented as async");
    assert_eq!(op.id.to_string(), id);
}

#[tokio::test]
async fn delete_image_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .delete_image("abc123")
        .await
        .expect("delete_image should return an Operation");
    assert_eq!(op.id.to_string(), id);
}

#[tokio::test]
async fn update_image_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .update_image("abc123", &serde_json::json!({"public": true}))
        .await
        .expect("update_image should return an Operation, per the crate-wide convention");
    assert_eq!(op.id.to_string(), id);
}

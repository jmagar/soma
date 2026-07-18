use crate::config::ClientConfig;
use crate::transport::{
    unix::tests::{json_response, spawn_fake_daemon},
    Client, WithEtag,
};

fn network_json(name: &str) -> String {
    format!(
        r#"{{"type":"sync","status":"Success","status_code":200,"metadata":{{"name":"{name}","type":"bridge","managed":true,"status":"Created","config":{{}}}}}}"#
    )
}

fn operation_json(id: &str) -> String {
    format!(
        r#"{{"type":"async","status":"Operation created","status_code":100,"operation":"/1.0/operations/{id}","metadata":{{"id":"{id}","class":"task","status":"Running","status_code":103,"resources":{{}},"may_cancel":true,"err":null}}}}"#
    )
}

#[tokio::test]
async fn get_network_deserializes_the_documented_shape_and_returns_etag() {
    let body = network_json("incusbr0");
    let (socket_path, _dir) = spawn_fake_daemon(move |_req| {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nETag: \"net-etag\"\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        response.into_bytes()
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let WithEtag {
        value: network,
        etag,
    } = client
        .get_network("incusbr0")
        .await
        .expect("get_network should succeed");

    assert_eq!(network.name, "incusbr0");
    assert_eq!(network.network_type, "bridge");
    assert_eq!(etag.as_deref(), Some("\"net-etag\""));
}

#[tokio::test]
async fn list_networks_requires_explicit_recursion() {
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
        .list_networks(true)
        .await
        .expect("list_networks should succeed");
    assert!(seen_request.lock().unwrap().contains("recursion=true"));
}

#[tokio::test]
async fn create_and_delete_network_return_operations() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let create_op = client
        .create_network(&serde_json::json!({"name": "br1", "type": "bridge"}))
        .await
        .expect("create_network should return an Operation, per the crate-wide convention");
    assert_eq!(create_op.id.to_string(), id);

    let delete_op = client
        .delete_network("br1")
        .await
        .expect("delete_network should return an Operation");
    assert_eq!(delete_op.id.to_string(), id);
}

#[tokio::test]
async fn update_network_sends_if_match_header_when_etag_is_provided() {
    let seen_request = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let seen = seen_request.clone();
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) = spawn_fake_daemon(move |req| {
        *seen.lock().unwrap() = String::from_utf8_lossy(&req).into_owned();
        json_response("HTTP/1.1 202 Accepted", &body)
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .update_network(
            "br1",
            &serde_json::json!({"config": {}}),
            Some("\"net-etag\""),
        )
        .await
        .expect("update_network should succeed");
    assert_eq!(op.id.to_string(), id);

    assert!(seen_request
        .lock()
        .unwrap()
        .contains("If-Match: \"net-etag\""));
}

#[tokio::test]
async fn update_network_maps_412_to_precondition_failed() {
    let body = r#"{"type":"error","error":"stale etag","error_code":412}"#;
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 412 Precondition Failed", body))
            .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let err = client
        .update_network("br1", &serde_json::json!({}), Some("\"stale\""))
        .await
        .expect_err("412 must map to a distinguishable error");

    assert!(matches!(
        err,
        crate::Error::PreconditionFailed { resource } if resource == "br1"
    ));
}

use super::*;
use crate::config::ClientConfig;
use crate::transport::{
    unix::tests::{json_response, spawn_fake_daemon},
    Client,
};

fn instance_json(name: &str) -> String {
    format!(
        r#"{{"type":"sync","status":"Success","status_code":200,"metadata":{{"name":"{name}","status":"Running","status_code":103,"type":"container","architecture":"x86_64","created_at":"2026-01-01T00:00:00Z","last_used_at":"2026-01-01T00:00:00Z","location":"none","project":"default","config":{{}},"devices":{{}},"profiles":["default"]}}}}"#
    )
}

fn operation_json(id: &str) -> String {
    format!(
        r#"{{"type":"async","status":"Operation created","status_code":100,"operation":"/1.0/operations/{id}","metadata":{{"id":"{id}","class":"task","status":"Running","status_code":103,"resources":{{}},"may_cancel":true,"err":null}}}}"#
    )
}

#[tokio::test]
async fn get_instance_deserializes_the_documented_shape_and_returns_etag() {
    let body = instance_json("c1");
    let (socket_path, _dir) = spawn_fake_daemon(move |_req| {
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nETag: \"abc123\"\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        response.into_bytes()
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let WithEtag {
        value: instance,
        etag,
    } = client
        .get_instance("c1")
        .await
        .expect("get_instance should succeed");

    assert_eq!(instance.name, "c1");
    assert_eq!(instance.instance_type, "container");
    assert_eq!(etag.as_deref(), Some("\"abc123\""));
}

#[tokio::test]
async fn list_instances_passes_recursion_through_as_a_query_param() {
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
        .list_instances(true)
        .await
        .expect("list_instances should succeed");

    assert!(seen_request.lock().unwrap().contains("recursion=true"));
}

#[tokio::test]
async fn create_instance_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let params = CreateInstanceParams {
        name: "c1".to_owned(),
        instance_type: "container".to_owned(),
        source: serde_json::json!({"type": "image", "fingerprint": "abc123"}),
    };
    let op: Operation = client
        .create_instance(&params)
        .await
        .expect("create_instance should return an Operation");

    assert_eq!(op.id.to_string(), id);
}

#[tokio::test]
async fn update_instance_sends_if_match_header_when_etag_is_provided() {
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

    client
        .update_instance("c1", &serde_json::json!({"config": {}}), Some("\"abc123\""))
        .await
        .expect("update_instance should succeed");

    assert!(seen_request
        .lock()
        .unwrap()
        .contains("If-Match: \"abc123\""));
}

#[tokio::test]
async fn update_instance_maps_412_to_precondition_failed() {
    let body = r#"{"type":"error","error":"stale etag","error_code":412}"#;
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 412 Precondition Failed", body))
            .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let err = client
        .update_instance("c1", &serde_json::json!({}), Some("\"stale\""))
        .await
        .expect_err("412 must map to a distinguishable error");

    assert!(matches!(
        err,
        crate::Error::PreconditionFailed { resource } if resource == "c1"
    ));
}

#[tokio::test]
async fn patch_instance_maps_412_to_precondition_failed() {
    let body = r#"{"type":"error","error":"stale etag","error_code":412}"#;
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 412 Precondition Failed", body))
            .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let err = client
        .patch_instance("c1", &serde_json::json!({"config": {}}), Some("\"stale\""))
        .await
        .expect_err("412 must map to a distinguishable error on PATCH too");

    assert!(matches!(err, crate::Error::PreconditionFailed { .. }));
}

#[tokio::test]
async fn delete_instance_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .delete_instance("c1")
        .await
        .expect("delete should succeed");
    assert_eq!(op.id.to_string(), id);
}

#[tokio::test]
async fn state_transitions_return_operations() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    for op_result in [
        client.start_instance("c1").await,
        client.stop_instance("c1").await,
        client.restart_instance("c1").await,
        client.pause_instance("c1").await,
    ] {
        let op = op_result.expect("state transition should return an Operation");
        assert_eq!(op.id.to_string(), id);
    }
}

#[tokio::test]
async fn snapshot_create_and_delete_return_operations() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let create_op = client
        .create_snapshot("c1", "snap1")
        .await
        .expect("create_snapshot should return an Operation");
    assert_eq!(create_op.id.to_string(), id);

    let delete_op = client
        .delete_snapshot("c1", "snap1")
        .await
        .expect("delete_snapshot should return an Operation");
    assert_eq!(delete_op.id.to_string(), id);
}

#[tokio::test]
async fn list_snapshots_deserializes_a_list() {
    let body = r#"{"type":"sync","status":"Success","status_code":200,"metadata":["/1.0/instances/c1/snapshots/snap1"]}"#;
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 200 OK", body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let snapshots = client
        .list_snapshots("c1", false)
        .await
        .expect("list_snapshots should succeed");
    assert_eq!(snapshots.len(), 1);
}

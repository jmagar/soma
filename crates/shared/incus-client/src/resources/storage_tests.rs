use crate::config::ClientConfig;
use crate::transport::{
    unix::tests::{json_response, spawn_fake_daemon},
    Client,
};

fn pool_json(name: &str) -> String {
    format!(
        r#"{{"type":"sync","status":"Success","status_code":200,"metadata":{{"name":"{name}","driver":"zfs","status":"Created","config":{{}}}}}}"#
    )
}

/// `recursion=false` shape: Incus returns bare volume URL strings, not
/// typed objects.
fn volume_list_urls_json() -> &'static str {
    r#"{"type":"sync","status":"Success","status_code":200,"metadata":["/1.0/storage-pools/default/volumes/custom/vol1"]}"#
}

/// `recursion=true` shape: Incus returns full volume objects.
fn volume_list_objects_json() -> &'static str {
    r#"{"type":"sync","status":"Success","status_code":200,"metadata":[{"name":"vol1","type":"custom","content_type":"filesystem","config":{}}]}"#
}

fn operation_json(id: &str) -> String {
    format!(
        r#"{{"type":"async","status":"Operation created","status_code":100,"operation":"/1.0/operations/{id}","metadata":{{"id":"{id}","class":"task","status":"Running","status_code":103,"resources":{{}},"may_cancel":true,"err":null}}}}"#
    )
}

fn volume_json(name: &str) -> String {
    format!(
        r#"{{"type":"sync","status":"Success","status_code":200,"metadata":{{"name":"{name}","type":"custom","content_type":"filesystem","config":{{}}}}}}"#
    )
}

#[tokio::test]
async fn get_storage_pool_deserializes_the_documented_shape() {
    let body = pool_json("default");
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 200 OK", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let pool = client
        .get_storage_pool("default")
        .await
        .expect("get_storage_pool should succeed");
    assert_eq!(pool.name, "default");
    assert_eq!(pool.driver, "zfs");
}

/// Mocks a fake daemon that returns the Incus-documented shape for each
/// recursion level: a bare URL-string array for `recursion=false`, full
/// volume objects for `recursion=true`. Real callers must be able to
/// deserialize both, since `list_storage_volumes` returns untyped
/// `serde_json::Value`s (see the P1 fix this test guards against: this
/// method used to be typed `Vec<StorageVolume>`, which panics on the
/// `recursion=false` wire shape).
async fn spawn_recursion_aware_volume_daemon() -> (std::path::PathBuf, tempfile::TempDir) {
    spawn_fake_daemon(move |req| {
        let req_text = String::from_utf8_lossy(&req);
        if req_text.contains("recursion=true") {
            json_response("HTTP/1.1 200 OK", volume_list_objects_json())
        } else {
            json_response("HTTP/1.1 200 OK", volume_list_urls_json())
        }
    })
    .await
}

#[tokio::test]
async fn list_storage_volumes_is_scoped_to_a_specific_pool() {
    let seen_request = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let seen = seen_request.clone();
    let (socket_path, _dir) = spawn_fake_daemon(move |req| {
        *seen.lock().unwrap() = String::from_utf8_lossy(&req).into_owned();
        json_response("HTTP/1.1 200 OK", volume_list_objects_json())
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let volumes = client
        .list_storage_volumes("default", true)
        .await
        .expect("list_storage_volumes should succeed");

    assert_eq!(volumes.len(), 1);
    assert_eq!(volumes[0]["name"], "vol1");
    assert!(seen_request
        .lock()
        .unwrap()
        .contains("/1.0/storage-pools/default/volumes"));
}

#[tokio::test]
async fn list_storage_volumes_with_recursion_false_deserializes_bare_url_strings() {
    let (socket_path, _dir) = spawn_recursion_aware_volume_daemon().await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let volumes = client
        .list_storage_volumes("default", false)
        .await
        .expect("recursion=false must deserialize as bare URL strings, not typed objects");

    assert_eq!(volumes.len(), 1);
    assert_eq!(
        volumes[0],
        serde_json::json!("/1.0/storage-pools/default/volumes/custom/vol1")
    );
}

#[tokio::test]
async fn list_storage_volumes_with_recursion_true_deserializes_full_objects() {
    let (socket_path, _dir) = spawn_recursion_aware_volume_daemon().await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let volumes = client
        .list_storage_volumes("default", true)
        .await
        .expect("recursion=true must deserialize as full volume objects");

    assert_eq!(volumes.len(), 1);
    assert_eq!(volumes[0]["name"], "vol1");
    assert_eq!(volumes[0]["type"], "custom");
}

#[tokio::test]
async fn create_and_delete_storage_pool_return_operations() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let create_op = client
        .create_storage_pool(&serde_json::json!({"name": "pool1", "driver": "zfs"}))
        .await
        .expect("create_storage_pool should return an Operation");
    assert_eq!(create_op.id.to_string(), id);

    let delete_op = client
        .delete_storage_pool("pool1")
        .await
        .expect("delete_storage_pool should return an Operation");
    assert_eq!(delete_op.id.to_string(), id);
}

#[tokio::test]
async fn update_storage_pool_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 202 Accepted", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .update_storage_pool("pool1", &serde_json::json!({"description": "updated"}))
        .await
        .expect("update_storage_pool should return an Operation");
    assert_eq!(op.id.to_string(), id);
}

#[tokio::test]
async fn get_storage_volume_deserializes_the_documented_shape() {
    let body = volume_json("vol1");
    let (socket_path, _dir) =
        spawn_fake_daemon(move |_req| json_response("HTTP/1.1 200 OK", &body)).await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let volume = client
        .get_storage_volume("default", "custom", "vol1")
        .await
        .expect("get_storage_volume should succeed");
    assert_eq!(volume.name, "vol1");
    assert_eq!(volume.volume_type, "custom");
}

#[tokio::test]
async fn update_storage_volume_returns_an_operation() {
    let id = uuid::Uuid::new_v4().to_string();
    let body = operation_json(&id);
    let seen_request = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let seen = seen_request.clone();
    let (socket_path, _dir) = spawn_fake_daemon(move |req| {
        *seen.lock().unwrap() = String::from_utf8_lossy(&req).into_owned();
        json_response("HTTP/1.1 202 Accepted", &body)
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let op = client
        .update_storage_volume(
            "default",
            "custom",
            "vol1",
            &serde_json::json!({"config": {}}),
        )
        .await
        .expect("update_storage_volume should return an Operation");
    assert_eq!(op.id.to_string(), id);
    assert!(seen_request
        .lock()
        .unwrap()
        .contains("/1.0/storage-pools/default/volumes/custom/vol1"));
}

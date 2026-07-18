use super::*;
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

fn volume_list_json() -> &'static str {
    r#"{"type":"sync","status":"Success","status_code":200,"metadata":[{"name":"vol1","type":"custom","content_type":"filesystem","config":{}}]}"#
}

fn operation_json(id: &str) -> String {
    format!(
        r#"{{"type":"async","status":"Operation created","status_code":100,"operation":"/1.0/operations/{id}","metadata":{{"id":"{id}","class":"task","status":"Running","status_code":103,"resources":{{}},"may_cancel":true,"err":null}}}}"#
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

#[tokio::test]
async fn list_storage_volumes_is_scoped_to_a_specific_pool() {
    let seen_request = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let seen = seen_request.clone();
    let (socket_path, _dir) = spawn_fake_daemon(move |req| {
        *seen.lock().unwrap() = String::from_utf8_lossy(&req).into_owned();
        json_response("HTTP/1.1 200 OK", volume_list_json())
    })
    .await;
    let client = Client::new(ClientConfig::unix_socket(socket_path));

    let volumes = client
        .list_storage_volumes("default", false)
        .await
        .expect("list_storage_volumes should succeed");

    assert_eq!(volumes.len(), 1);
    assert_eq!(volumes[0].name, "vol1");
    assert!(seen_request
        .lock()
        .unwrap()
        .contains("/1.0/storage-pools/default/volumes"));
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

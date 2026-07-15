use super::SERVER_INSTRUCTIONS;

#[test]
fn server_instructions_point_clients_to_schema_resource() {
    assert!(SERVER_INSTRUCTIONS.contains("soma://schema/mcp-tool"));
    assert!(SERVER_INSTRUCTIONS.contains("Responses are structured JSON"));
}

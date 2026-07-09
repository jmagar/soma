//! Unit tests for src/mcp/prompts.rs

use super::*;
use rtemplate_contracts::providers::{
    ProviderCatalog, ProviderIdentity, ProviderKind, ProviderPrompt,
};
use serde_json::json;

#[test]
fn list_prompts_returns_quick_start() {
    let result = list_prompts();
    let names: Vec<&str> = result.prompts.iter().map(|p| p.name.as_str()).collect();
    assert!(
        names.contains(&"quick_start"),
        "expected quick_start prompt"
    );
}

#[test]
fn get_prompt_quick_start_returns_message() {
    let result = get_prompt(rmcp::model::GetPromptRequestParams::new("quick_start"))
        .expect("quick_start should resolve");
    assert!(
        !result.messages.is_empty(),
        "prompt should have at least one message"
    );
}

#[test]
fn get_prompt_unknown_returns_err() {
    let result = get_prompt(rmcp::model::GetPromptRequestParams::new("nonexistent"));
    assert!(result.is_err(), "unknown prompt should return Err");
}

#[test]
fn provider_prompts_are_listed_and_return_markdown_template() {
    let catalogs = vec![ProviderCatalog {
        schema_version: 1,
        provider: ProviderIdentity {
            name: "review-prompt".to_owned(),
            kind: ProviderKind::StaticRust,
            title: None,
            description: None,
            homepage: None,
            source: None,
            version: None,
            enabled: None,
        },
        tools: Vec::new(),
        prompts: vec![ProviderPrompt {
            name: "review".to_owned(),
            description: "Review prompt".to_owned(),
            template: Some("# Review\n\nCheck correctness.".to_owned()),
            arguments_schema: None,
            scope: None,
            mcp: None,
            examples: Vec::new(),
        }],
        resources: Vec::new(),
        tasks: Vec::new(),
        elicitation: Vec::new(),
        env: Vec::new(),
        capabilities: Default::default(),
        docs: None,
        plugin: None,
        ui: None,
        meta: json!({}),
    }];

    let prompts = provider_prompts(&catalogs);
    assert_eq!(prompts.len(), 1);
    assert_eq!(prompts[0].name, "review");

    let result = get_provider_prompt(
        &catalogs,
        &rmcp::model::GetPromptRequestParams::new("review"),
    )
    .expect("provider prompt should resolve");
    let value = serde_json::to_value(result).expect("prompt result should serialize");
    assert_eq!(
        value["messages"][0]["content"]["text"],
        "# Review\n\nCheck correctness."
    );
}

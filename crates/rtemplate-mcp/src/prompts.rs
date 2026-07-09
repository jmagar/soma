//! MCP prompts for the example server.
//!
//! Prompts are pre-canned message templates that MCP clients can invoke.
//! They appear in the "Prompts" section of compatible MCP UIs.
//!
//! **Template**: replace `quick_start` with prompts relevant to your domain.

use rmcp::model::{
    GetPromptRequestParams, GetPromptResult, ListPromptsResult, Prompt, PromptMessage, Role,
};
use rtemplate_contracts::providers::{ProviderCatalog, ProviderPrompt};

pub(super) fn list_prompts() -> ListPromptsResult {
    ListPromptsResult {
        prompts: vec![Prompt::new(
            "quick_start",
            Some(
                "Check the server status and get a personalised greeting to verify \
                 the MCP connection is working end-to-end.",
            ),
            None,
        )],
        ..Default::default()
    }
}

pub(super) fn get_prompt(request: GetPromptRequestParams) -> anyhow::Result<GetPromptResult> {
    match request.name.as_str() {
        "quick_start" => Ok(GetPromptResult::new(vec![PromptMessage::new_text(
            Role::User,
            "Use the example tool with action=status to check the server is running, \
             then use action=greet with your name to get a personalised greeting. \
             Report back both results.",
        )])
        .with_description("Verify the MCP server is working with a status check and greeting")),
        other => Err(anyhow::anyhow!("unknown prompt: {other}")),
    }
}

pub(super) fn provider_prompts(catalogs: &[ProviderCatalog]) -> Vec<Prompt> {
    catalogs
        .iter()
        .flat_map(|catalog| &catalog.prompts)
        .filter(|prompt| prompt_enabled(prompt))
        .map(|prompt| Prompt::new(prompt.name.clone(), Some(prompt.description.clone()), None))
        .collect()
}

pub(super) fn get_provider_prompt(
    catalogs: &[ProviderCatalog],
    request: &GetPromptRequestParams,
) -> Option<GetPromptResult> {
    catalogs
        .iter()
        .flat_map(|catalog| &catalog.prompts)
        .find(|prompt| prompt_enabled(prompt) && prompt.name == request.name)
        .map(|prompt| {
            GetPromptResult::new(vec![PromptMessage::new_text(
                Role::User,
                prompt.template.clone().unwrap_or_default(),
            )])
            .with_description(prompt.description.clone())
        })
}

fn prompt_enabled(prompt: &ProviderPrompt) -> bool {
    prompt.mcp.as_ref().map(|mcp| mcp.enabled).unwrap_or(true)
}

#[cfg(test)]
#[path = "prompts_tests.rs"]
mod tests;

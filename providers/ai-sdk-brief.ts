export default {
  "schema_version": 1,
  "provider": {
    "name": "local-ai-sdk-tools",
    "kind": "ai-sdk",
    "title": "Local AI SDK Tools",
    "description": "Self-contained AI SDK sidecar tools for Soma provider smoke tests."
  },
  "tools": [
    {
      "name": "ai_sdk_brief",
      "title": "AI SDK Brief",
      "description": "Create a compact brief from text through Soma's AI SDK sidecar runtime.",
      "input_schema": {
        "type": "object",
        "additionalProperties": false,
        "required": ["text"],
        "properties": {
          "text": {
            "type": "string",
            "minLength": 1,
            "description": "Text to condense."
          },
          "max_words": {
            "type": "integer",
            "minimum": 4,
            "maximum": 40,
            "default": 18,
            "description": "Maximum words in the generated brief."
          }
        }
      },
      "output_schema": {
        "type": "object",
        "additionalProperties": false,
        "required": ["brief", "word_count", "input_chars", "runtime"],
        "properties": {
          "brief": { "type": "string" },
          "word_count": { "type": "integer" },
          "input_chars": { "type": "integer" },
          "runtime": { "type": "string" }
        }
      },
      "cli": {
        "enabled": true,
        "command": "ai-sdk-brief"
      },
      "rest": {
        "enabled": true,
        "method": "POST",
        "path": "/v1/providers/ai-sdk-brief"
      },
      "palette": {
        "enabled": true,
        "category": "AI"
      }
    }
  ]
};

export async function call(input) {
  const text = String(input.params?.text ?? "").trim();
  const maxWords = Math.min(Math.max(Number(input.params?.max_words ?? 18), 4), 40);
  const words = text.split(/\s+/).filter(Boolean);
  const briefWords = words.slice(0, maxWords);
  const brief = briefWords.join(" ") + (words.length > maxWords ? "..." : "");

  return {
    brief,
    word_count: words.length,
    input_chars: text.length,
    runtime: "ai-sdk-sidecar"
  };
}

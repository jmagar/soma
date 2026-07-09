export default {
  "schema_version": 1,
  "provider": {
    "name": "hello-ai-sdk",
    "kind": "ai-sdk",
    "title": "Hello AI SDK",
    "version": "0.1.0"
  },
  "tools": [
    {
      "name": "hello_ai_sdk",
      "description": "AI SDK TypeScript provider example.",
      "input_schema": {
        "type": "object",
        "properties": {
          "message": { "type": "string" }
        },
        "additionalProperties": false
      },
      "output_schema": {
        "type": "object",
        "additionalProperties": true
      },
      "cli": {
        "enabled": true,
        "command": "hello_ai_sdk",
        "about": "Run the AI SDK provider example."
      },
      "rest": {
        "enabled": true,
        "method": "POST",
        "path": "/v1/providers/hello_ai_sdk",
        "summary": "Run the AI SDK provider example."
      }
    }
  ],
  "meta": {
    "example": true
  }
};

export async function call(input) {
  return {
    ok: true,
    echoed: input.message ?? null
  };
}

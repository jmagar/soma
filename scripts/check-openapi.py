#!/usr/bin/env python3
"""Generate and verify OpenAPI docs for the template REST API."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
CARGO = ROOT / "Cargo.toml"
ACTIONS = ROOT / "src/actions.rs"
OUT = ROOT / "docs/generated/openapi.json"

REST_ENDPOINT = "/v1/example"

# Action-specific param examples. Actions not listed here get an empty params object.
_PARAM_EXAMPLES: dict[str, dict] = {
    "greet": {"name": "Alice"},
    "echo": {"message": "Hello!"},
    "config_get": {"key": "mcp.host"},
    "config_set": {"key": "mcp.host", "value": "0.0.0.0"},
    "config_unset": {"key": "mcp.host"},
}


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def package_version() -> str:
    text = read(CARGO)
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    if not match:
        raise RuntimeError("could not find package version in Cargo.toml")
    return match.group(1)


def action_entries() -> list[dict[str, Any]]:
    text = read(ACTIONS)
    entries = re.findall(r"ActionSpec\s*\{(.*?)\}", text, re.S)
    actions: list[dict[str, Any]] = []
    for entry in entries:
        name_match = re.search(r'name:\s*"([^"]+)"', entry)
        scope_match = re.search(r"required_scope:\s*([^,\n]+)", entry)
        rest_match = re.search(r"rest_enabled:\s*(true|false)", entry)
        mcp_match = re.search(r"mcp_enabled:\s*(true|false)", entry)
        if not name_match or not scope_match or not rest_match or not mcp_match:
            continue
        scope_expr = scope_match.group(1).strip()
        if scope_expr == "None":
            scope = "public"
        elif scope_expr == "Some(READ_SCOPE)":
            scope = "example:read"
        elif scope_expr == "Some(WRITE_SCOPE)":
            scope = "example:write"
        else:
            scope = "example:__deny__"
        actions.append(
            {
                "name": name_match.group(1),
                "scope": scope,
                "rest_enabled": rest_match.group(1) == "true",
                "mcp_enabled": mcp_match.group(1) == "true",
            }
        )
    return actions


def rest_actions() -> list[dict[str, Any]]:
    return [action for action in action_entries() if action["rest_enabled"]]


def schema_ref(name: str) -> dict[str, str]:
    return {"$ref": f"#/components/schemas/{name}"}


def render() -> dict[str, Any]:
    actions = rest_actions()
    action_names = [action["name"] for action in actions]
    version = package_version()
    return {
        "openapi": "3.1.0",
        "info": {
            "title": "Example MCP REST API",
            "version": version,
            "description": (
                "Generated OpenAPI schema for rmcp-template's REST surface. "
                "TEMPLATE: rename Example identifiers and action schemas when adapting. "
                "Auth modes: loopback/trusted-gateway deployments may have no local auth; "
                "mounted bearer mode uses EXAMPLE_MCP_TOKEN; OAuth mode uses bearer JWTs. "
                "REST actions require their action-specific scopes when auth is mounted."
            ),
        },
        "servers": [
            {
                "url": "http://localhost:3100",
                "description": "Default local development server",
            }
        ],
        "tags": [
            {"name": "health", "description": "Unauthenticated runtime probes"},
            {"name": "actions", "description": "REST action dispatch"},
        ],
        "paths": {
            "/health": {
                "get": {
                    "tags": ["health"],
                    "summary": "Liveness probe",
                    "operationId": "getHealth",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Server is alive",
                            "content": {
                                "application/json": {
                                    "schema": schema_ref("HealthResponse"),
                                    "examples": {"ok": {"value": {"status": "ok"}}},
                                }
                            },
                        }
                    },
                }
            },
            "/openapi.json": {
                "get": {
                    "tags": ["health"],
                    "summary": "OpenAPI schema",
                    "operationId": "getOpenApiSchema",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Generated OpenAPI schema for the REST surface",
                            "content": {
                                "application/json": {
                                    "schema": {"type": "object", "additionalProperties": True}
                                }
                            },
                        }
                    },
                }
            },
            "/status": {
                "get": {
                    "tags": ["health"],
                    "summary": "Runtime status",
                    "operationId": "getStatus",
                    "security": [],
                    "responses": {
                        "200": {
                            "description": "Runtime status with secrets redacted",
                            "content": {"application/json": {"schema": schema_ref("StatusResponse")}},
                        },
                        "500": {"$ref": "#/components/responses/InternalError"},
                    },
                }
            },
            REST_ENDPOINT: {
                "post": {
                    "tags": ["actions"],
                    "summary": "Dispatch a REST action",
                    "description": (
                        "Thin REST shim over the shared service layer. MCP-only actions are "
                        "not exposed here. Current REST actions: " + ", ".join(action_names) + ". "
                        "When auth is mounted, each action requires its declared scope; "
                        "example:write satisfies example:read."
                    ),
                    "operationId": "dispatchExampleAction",
                    "security": [{"BearerAuth": []}],
                    "requestBody": {
                        "required": True,
                        "content": {
                            "application/json": {
                                "schema": schema_ref("ActionRequest"),
                                "examples": {
                                    action["name"]: {
                                        "value": {
                                            "action": action["name"],
                                            "params": _PARAM_EXAMPLES.get(action["name"], {}),
                                        }
                                    }
                                    for action in actions
                                },
                            }
                        },
                    },
                    "responses": {
                        "200": {
                            "description": "Action result. Shape depends on the requested action.",
                            "content": {"application/json": {"schema": schema_ref("ActionResponse")}},
                        },
                        "400": {"$ref": "#/components/responses/BadRequest"},
                        "401": {"$ref": "#/components/responses/Unauthorized"},
                        "403": {"$ref": "#/components/responses/Forbidden"},
                        "500": {"$ref": "#/components/responses/InternalError"},
                    },
                }
            },
        },
        "components": {
            "securitySchemes": {
                "BearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "opaque",
                    "description": "Static bearer token in bearer mode; OAuth mode also uses bearer JWTs. Loopback and trusted-gateway modes may not require local auth.",
                }
            },
            "schemas": {
                "ActionName": {
                    "type": "string",
                    "enum": action_names,
                    "description": "REST-capable action names from src/actions.rs.",
                },
                "ActionRequest": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["action"],
                    "properties": {
                        "action": schema_ref("ActionName"),
                        "params": {
                            "type": "object",
                            "description": "Action-specific parameters. greet.name is optional; echo.message is required.",
                            "additionalProperties": True,
                            "default": {},
                        },
                    },
                },
                "ActionResponse": {
                    "oneOf": [
                        schema_ref("GreetResponse"),
                        schema_ref("EchoResponse"),
                        schema_ref("StatusResponse"),
                        schema_ref("HelpResponse"),
                    ]
                },
                "GreetResponse": {
                    "type": "object",
                    "required": ["greeting", "target"],
                    "properties": {
                        "greeting": {"type": "string"},
                        "target": {"type": "string"},
                        "server": {"type": "string"},
                    },
                    "additionalProperties": True,
                },
                "EchoResponse": {
                    "type": "object",
                    "required": ["echo"],
                    "properties": {"echo": {"type": "string"}},
                    "additionalProperties": True,
                },
                "StatusResponse": {
                    "type": "object",
                    "required": ["status"],
                    "properties": {
                        "status": {"type": "string"},
                        "api_url": {"type": "string"},
                        "note": {"type": "string"},
                        "server": {"type": "string"},
                        "version": {"type": "string"},
                        "transport": {"type": "string"},
                    },
                    "additionalProperties": True,
                },
                "HealthResponse": {
                    "type": "object",
                    "required": ["status"],
                    "properties": {"status": {"type": "string", "const": "ok"}},
                    "additionalProperties": False,
                },
                "HelpResponse": {
                    "type": "object",
                    "required": ["actions", "mcp_actions", "usage", "examples"],
                    "properties": {
                        "actions": {"type": "array", "items": schema_ref("ActionName")},
                        "mcp_actions": {"type": "array", "items": {"type": "string"}},
                        "usage": {"type": "string"},
                        "examples": {"type": "object", "additionalProperties": True},
                    },
                    "additionalProperties": True,
                },
                "ErrorResponse": {
                    "type": "object",
                    "required": ["error"],
                    "properties": {"error": {"type": "string"}},
                    "additionalProperties": False,
                },
            },
            "responses": {
                "BadRequest": {
                    "description": "Validation error",
                    "content": {"application/json": {"schema": schema_ref("ErrorResponse")}},
                },
                "Unauthorized": {
                    "description": "Missing or invalid authentication",
                    "content": {"application/json": {"schema": schema_ref("ErrorResponse")}},
                },
                "Forbidden": {
                    "description": "Authenticated request lacks the required scope",
                    "content": {"application/json": {"schema": schema_ref("ErrorResponse")}},
                },
                "InternalError": {
                    "description": "Internal server error",
                    "content": {"application/json": {"schema": schema_ref("ErrorResponse")}},
                },
            },
        },
        "x-template": {
            "source": "scripts/check-openapi.py",
            "action_metadata": "src/actions.rs",
            "rest_actions": action_names,
            "mcp_actions": [
                action["name"] for action in action_entries() if action["mcp_enabled"]
            ],
            "rest_only_actions": [
                action["name"]
                for action in action_entries()
                if action["rest_enabled"] and not action["mcp_enabled"]
            ],
            "mcp_only_actions": [
                action["name"]
                for action in action_entries()
                if action["mcp_enabled"] and not action["rest_enabled"]
            ],
        },
    }


def canonical_json(value: dict[str, Any]) -> str:
    return json.dumps(value, indent=2, sort_keys=False) + "\n"


def validate_openapi(value: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    if value.get("openapi") != "3.1.0":
        failures.append("OpenAPI version must be 3.1.0")
    for path in ["/health", "/openapi.json", "/status", REST_ENDPOINT]:
        if path not in value.get("paths", {}):
            failures.append(f"missing path {path}")
    for path, methods in value.get("paths", {}).items():
        for method, operation in methods.items():
            if not operation.get("operationId"):
                failures.append(f"{method.upper()} {path} is missing operationId")
    action_enum = value.get("components", {}).get("schemas", {}).get("ActionName", {}).get("enum")
    expected = [action["name"] for action in rest_actions()]
    if action_enum != expected:
        failures.append(f"ActionName enum drifted: expected {expected}, got {action_enum}")
    mcp_only = {
        a["name"]
        for a in action_entries()
        if a["mcp_enabled"] and not a["rest_enabled"]
    }
    for name in sorted(mcp_only):
        if name in (action_enum or []):
            failures.append(f"MCP-only action {name!r} must not appear in REST ActionName enum")
    return failures


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--write", action="store_true", help="Rewrite docs/generated/openapi.json")
    parser.add_argument("--check", action="store_true", help="Fail if generated OpenAPI is stale")
    args = parser.parse_args()
    if not args.write and not args.check:
        args.check = True

    rendered = canonical_json(render())
    failures = validate_openapi(json.loads(rendered))

    if args.write:
        OUT.parent.mkdir(parents=True, exist_ok=True)
        OUT.write_text(rendered, encoding="utf-8")
        print(f"wrote {OUT.relative_to(ROOT)}")

    if args.check:
        if not OUT.exists():
            failures.append("docs/generated/openapi.json is missing; run scripts/check-openapi.py --write")
        elif OUT.read_text(encoding="utf-8") != rendered:
            failures.append("docs/generated/openapi.json is stale; run scripts/check-openapi.py --write")

    if failures:
        for failure in failures:
            print(f"FAIL: {failure}", file=sys.stderr)
        return 1
    if args.check:
        print("OpenAPI schema is current")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

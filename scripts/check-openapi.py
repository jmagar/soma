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
CARGO = ROOT / "crates/rmcp-template/Cargo.toml"
ACTIONS = ROOT / "crates/rtemplate-contracts/src/actions.rs"
CONFIG = ROOT / "crates/rtemplate-contracts/src/config.rs"
OUT = ROOT / "docs/generated/openapi.json"

LEGACY_REST_ENDPOINT = "/v1/example"
REST_SCHEMAS = {
    "greet": ("GreetRequest", "GreetResponse"),
    "echo": ("EchoRequest", "EchoResponse"),
    "status": (None, "StatusResponse"),
    "help": (None, "HelpResponse"),
}

# Action-specific param examples. Actions not listed here get an empty params object.
_PARAM_EXAMPLES: dict[str, dict] = {
    "greet": {"name": "Alice"},
    "echo": {"message": "Hello!"},
}


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def package_version() -> str:
    text = read(CARGO)
    match = re.search(r'^version\s*=\s*"([^"]+)"', text, re.M)
    if not match:
        raise RuntimeError("could not find package version in Cargo.toml")
    return match.group(1)


def default_mcp_port() -> int:
    text = read(CONFIG)
    match = re.search(r"fn default_mcp_port\(\).*?\{\s*(\d+)\s*\}", text, re.S)
    if not match:
        raise RuntimeError("could not find default_mcp_port in config.rs")
    return int(match.group(1))


def action_entries() -> list[dict[str, str]]:
    text = read(ACTIONS)
    entries = re.findall(r"ActionSpec\s*\{(.*?)\}", text, re.S)
    actions: list[dict[str, str]] = []
    for entry in entries:
        name_match = re.search(r'name:\s*"([^"]+)"', entry)
        scope_match = re.search(r"required_scope:\s*([^,\n]+)", entry)
        transport_match = re.search(r"transport:\s*ActionTransport::(\w+)", entry)
        rest_method_match = re.search(r'rest_method:\s*(None|Some\("[^"]*"\))', entry)
        rest_path_match = re.search(r'rest_path:\s*(None|Some\("[^"]*"\))', entry)
        cost_match = re.search(r"cost:\s*ActionCost::(\w+)", entry)
        if not name_match or not scope_match or not transport_match or not cost_match:
            continue
        rest_method = option_string(rest_method_match.group(1)) if rest_method_match else None
        rest_path = option_string(rest_path_match.group(1)) if rest_path_match else None
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
                "transport": transport_match.group(1),
                "rest_method": rest_method,
                "rest_path": rest_path,
                "cost": cost_match.group(1).lower(),
            }
        )
    return actions


def option_string(value: str) -> str | None:
    if value == "None":
        return None
    match = re.fullmatch(r'Some\("([^"]*)"\)', value)
    if not match:
        raise RuntimeError(f"unsupported option string expression: {value}")
    return match.group(1)


def action_spec_count() -> int:
    return len(re.findall(r"ActionSpec\s*\{\s*name:", read(ACTIONS)))


def rest_actions() -> list[dict[str, str]]:
    return [
        action
        for action in action_entries()
        if action["transport"] == "Any" and action.get("rest_method") and action.get("rest_path")
    ]


def schema_ref(name: str) -> dict[str, str]:
    return {"$ref": f"#/components/schemas/{name}"}


def render() -> dict[str, Any]:
    actions = rest_actions()
    action_names = [action["name"] for action in actions]
    version = package_version()
    port = default_mcp_port()
    paths = {
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
                "summary": "Local runtime status",
                "operationId": "getLocalStatus",
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
        "/v1/capabilities": {
            "get": {
                "tags": ["capabilities"],
                "summary": "Direct REST route inventory",
                "operationId": "getCapabilities",
                "security": [{"BearerAuth": []}, {}],
                "responses": {
                    "200": {
                        "description": "Supported direct REST routes and metadata",
                        "content": {"application/json": {"schema": schema_ref("CapabilitiesResponse")}},
                    },
                    "401": {"$ref": "#/components/responses/Unauthorized"},
                },
            }
        },
    }
    for action in actions:
        schemas = REST_SCHEMAS.get(action["name"])
        if schemas is None:
            continue
        method = action["rest_method"].lower()
        path = action["rest_path"]
        request_schema, response_schema = schemas
        operation: dict[str, Any] = {
            "tags": ["direct-rest"],
            "summary": f"Run {action['name']}",
            "description": "Direct REST route over the shared service layer.",
            "operationId": f"{method}{action['name'].title().replace('_', '')}",
            "security": [{"BearerAuth": []}, {}],
            "responses": {
                "200": {
                    "description": f"{action['name']} result",
                    "content": {"application/json": {"schema": schema_ref(response_schema)}},
                },
                "400": {"$ref": "#/components/responses/BadRequest"},
                "401": {"$ref": "#/components/responses/Unauthorized"},
                "403": {"$ref": "#/components/responses/Forbidden"},
                "500": {"$ref": "#/components/responses/InternalError"},
            },
        }
        if request_schema is not None:
            operation["requestBody"] = {
                "required": True,
                "content": {
                    "application/json": {
                        "schema": schema_ref(request_schema),
                        "examples": {
                            action["name"]: {
                                "value": _PARAM_EXAMPLES.get(action["name"], {}),
                            }
                        },
                    }
                },
            }
        paths[path] = {method: operation}
    paths[LEGACY_REST_ENDPOINT] = {
        "post": {
            "tags": ["legacy-actions"],
            "summary": "Deprecated action-envelope dispatch",
            "description": (
                "Compatibility shim for older clients. New application/platform servers "
                "should expose direct product REST routes and reserve action dispatch for MCP."
            ),
            "operationId": "dispatchExampleActionDeprecated",
            "deprecated": True,
            "security": [{"BearerAuth": []}, {}],
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
    }
    return {
        "openapi": "3.1.0",
        "info": {
            "title": "Example MCP REST API",
            "version": version,
            "description": (
                "Generated OpenAPI schema for rmcp-template's direct REST surface. "
                "TEMPLATE: rename Example identifiers and action schemas when adapting. "
                "Auth modes: loopback/trusted-gateway deployments may have no local auth; "
                "mounted bearer mode uses RTEMPLATE_MCP_TOKEN; OAuth mode uses bearer JWTs. "
                "REST actions require their action-specific scopes when auth is mounted."
            ),
        },
        "servers": [
            {
                "url": f"http://localhost:{port}",
                "description": "Default local development server",
            }
        ],
        "tags": [
            {"name": "health", "description": "Unauthenticated runtime probes"},
            {"name": "capabilities", "description": "REST route inventory"},
            {"name": "direct-rest", "description": "Preferred typed REST routes"},
            {"name": "legacy-actions", "description": "Deprecated action-envelope compatibility"},
        ],
        "paths": paths,
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
                    "description": "REST-capable action names from crates/rtemplate-contracts/src/actions.rs.",
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
                "GreetRequest": {
                    "type": "object",
                    "additionalProperties": False,
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Name to greet. Omit to greet the world.",
                        }
                    },
                },
                "EchoRequest": {
                    "type": "object",
                    "additionalProperties": False,
                    "required": ["message"],
                    "properties": {
                        "message": {
                            "type": "string",
                            "minLength": 1,
                            "description": "Message to echo back. Must not be empty.",
                        }
                    },
                },
                "ActionResponse": {
                    "oneOf": [
                        schema_ref("GreetResponse"),
                        schema_ref("EchoResponse"),
                        schema_ref("StatusResponse"),
                        schema_ref("HelpResponse"),
                        schema_ref("RestTruncationResponse"),
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
                "CapabilitiesResponse": {
                    "type": "object",
                    "required": [
                        "server",
                        "version",
                        "preferred_rest_style",
                        "supported_routes",
                        "routes",
                    ],
                    "properties": {
                        "server": {"type": "string"},
                        "version": {"type": "string"},
                        "preferred_rest_style": {
                            "type": "string",
                            "const": "direct_routes",
                        },
                        "supported_routes": {
                            "type": "array",
                            "items": {"type": "string"},
                        },
                        "routes": {
                            "type": "array",
                            "items": schema_ref("RestRoute"),
                        },
                    },
                    "additionalProperties": False,
                },
                "RestRoute": {
                    "type": "object",
                    "required": ["method", "path", "auth", "description"],
                    "properties": {
                        "method": {"type": "string"},
                        "path": {"type": "string"},
                        "action": {"type": ["string", "null"]},
                        "auth": {"type": "string"},
                        "description": {"type": "string"},
                    },
                    "additionalProperties": False,
                },
                "HelpResponse": {
                    "type": "object",
                    "required": ["actions", "mcp_only_actions", "usage", "examples"],
                    "properties": {
                        "actions": {"type": "array", "items": schema_ref("ActionName")},
                        "mcp_only_actions": {"type": "array", "items": {"type": "string"}},
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
                "RestTruncationResponse": {
                    "type": "object",
                    "required": ["truncated", "error", "max_response_bytes", "hint"],
                    "properties": {
                        "truncated": {"type": "boolean", "const": True},
                        "error": {
                            "type": "string",
                            "const": "response exceeded REST response size limit",
                        },
                        "max_response_bytes": {"type": "integer", "minimum": 1},
                        "hint": {"type": "string"},
                    },
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
            "action_metadata": "crates/rtemplate-contracts/src/actions.rs",
            "preferred_rest_style": "direct_routes",
            "rest_actions": action_names,
            "direct_rest_routes": {
                action["name"]: {
                    "method": action["rest_method"].upper(),
                    "path": action["rest_path"],
                }
                for action in actions
            },
            "action_costs": {action["name"]: action["cost"] for action in action_entries()},
            "mcp_only_actions": [action["name"] for action in action_entries() if action["transport"] == "McpOnly"],
        },
    }


def canonical_json(value: dict[str, Any]) -> str:
    return json.dumps(value, indent=2, sort_keys=False) + "\n"


def validate_openapi(value: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    if value.get("openapi") != "3.1.0":
        failures.append("OpenAPI version must be 3.1.0")
    required_paths = [
        "/health",
        "/openapi.json",
        "/status",
        "/v1/capabilities",
        *[action["rest_path"] for action in rest_actions()],
        LEGACY_REST_ENDPOINT,
    ]
    for path in required_paths:
        if path not in value.get("paths", {}):
            failures.append(f"missing path {path}")
    for path, methods in value.get("paths", {}).items():
        for method, operation in methods.items():
            if not operation.get("operationId"):
                failures.append(f"{method.upper()} {path} is missing operationId")
    action_enum = value.get("components", {}).get("schemas", {}).get("ActionName", {}).get("enum")
    entries = action_entries()
    if len(entries) != action_spec_count():
        failures.append(
            f"ActionSpec parser drifted: parsed {len(entries)} entries from {action_spec_count()} specs"
        )
    expected = [action["name"] for action in entries if action["transport"] == "Any"]
    if action_enum != expected:
        failures.append(f"ActionName enum drifted: expected {expected}, got {action_enum}")
    missing_route_metadata = [
        action["name"]
        for action in entries
        if action["transport"] == "Any"
        and (not action.get("rest_method") or not action.get("rest_path"))
    ]
    if missing_route_metadata:
        failures.append(
            f"ACTION_SPECS entries are missing REST route metadata: {missing_route_metadata}"
        )
    mcp_only = {a["name"] for a in entries if a["transport"] == "McpOnly"}
    for name in sorted(mcp_only):
        if name in (action_enum or []):
            failures.append(f"MCP-only action {name!r} must not appear in REST ActionName enum")
    x_template = value.get("x-template", {})
    if x_template.get("rest_actions") != expected:
        failures.append(
            f"x-template rest_actions drifted: expected {expected}, got {x_template.get('rest_actions')}"
        )
    expected_mcp_only = [
        action["name"] for action in entries if action["transport"] == "McpOnly"
    ]
    if x_template.get("mcp_only_actions") != expected_mcp_only:
        failures.append("x-template mcp_only_actions drifted")
    for action in rest_actions():
        method = action["rest_method"].lower()
        path = action["rest_path"]
        if method not in value.get("paths", {}).get(path, {}):
            failures.append(f"missing direct REST operation {method.upper()} {path}")
    legacy_operation = value.get("paths", {}).get(LEGACY_REST_ENDPOINT, {}).get("post", {})
    if legacy_operation.get("deprecated") is not True:
        failures.append(f"{LEGACY_REST_ENDPOINT} must be marked deprecated")
    rest_security = legacy_operation.get("security")
    if rest_security != [{"BearerAuth": []}, {}]:
        failures.append(
            f"{LEGACY_REST_ENDPOINT} security must document bearer auth and no-local-auth modes"
        )
    capabilities_schema = (
        value.get("paths", {})
        .get("/v1/capabilities", {})
        .get("get", {})
        .get("responses", {})
        .get("200", {})
        .get("content", {})
        .get("application/json", {})
        .get("schema", {})
    )
    if capabilities_schema != schema_ref("CapabilitiesResponse"):
        failures.append("/v1/capabilities must return CapabilitiesResponse")
    status_props = (
        value.get("components", {})
        .get("schemas", {})
        .get("StatusResponse", {})
        .get("properties", {})
    )
    if "api_url" in status_props:
        failures.append("StatusResponse must not advertise api_url on the public status schema")
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

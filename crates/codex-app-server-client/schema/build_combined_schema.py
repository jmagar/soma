"""
Builds a single self-contained, typify-friendly JSON Schema for the Codex
app-server v2 protocol, merging:
  - the v2-pruned bundle (ClientRequest/ServerNotification scoped to v2-only
    methods, flat #/definitions/X refs)
  - the master bundle's flat top-level definitions (JSON-RPC envelope,
    ServerRequest, ClientNotification, InitializeResponse, W3cTraceContext,
    and the approval/elicitation Params/Response types reachable from
    ServerRequest) with any "#/definitions/v2/X" refs rewritten to "#/definitions/X"
    so everything resolves in one flat namespace.

Also patches one schema (McpServerElicitationRequestParams) that combines a
top-level object (base properties) with a sibling `oneOf` where one branch
contains a wildcard (`true`) sub-schema - typify's schema-merge/disjointness
logic (typify-impl 0.7.0, merge.rs:427) panics with "not yet implemented" on
that specific shape. Flattening the shared base fields into each oneOf branch
(so it becomes a plain oneOf of self-contained objects, discriminated on the
existing "mode" literal) produces an equivalent, typify-friendly schema with
no loss of type fidelity.

Run from the generation directory that has both source bundles; see
CODEX_VERSION.txt for the exact `codex` version these were captured from.
"""

import json
import re
import sys
from pathlib import Path

GEN_DIR = Path(sys.argv[1]) if len(sys.argv) > 1 else Path(
    "/tmp/claude-1000/-home-jmagar-workspace-soma--claude-worktrees-codex-app-server-api-4798cc"
    "/2cde8de2-88fc-45dc-b917-ab2d33a1bd00/scratchpad/codex-v2-gen/experimental/schema"
)
OUT_PATH = Path(__file__).parent / "protocol.schema.json"


def rewrite_v2_refs(obj):
    if isinstance(obj, dict):
        out = {}
        for k, v in obj.items():
            if k == "$ref" and isinstance(v, str) and v.startswith("#/definitions/v2/"):
                out[k] = v.replace("#/definitions/v2/", "#/definitions/")
            else:
                out[k] = rewrite_v2_refs(v)
        return out
    if isinstance(obj, list):
        return [rewrite_v2_refs(x) for x in obj]
    return obj


def flatten_base_plus_oneof(schema):
    """Merge a schema's top-level object properties/required into each oneOf
    branch, producing a pure oneOf-of-self-contained-objects. Only applies
    when the schema actually has this shape; returns it unchanged otherwise."""
    if not (isinstance(schema, dict) and "oneOf" in schema and "properties" in schema):
        return schema
    base_props = schema.get("properties", {})
    base_required = schema.get("required", [])
    flattened = []
    for branch in schema["oneOf"]:
        merged_props = {**base_props, **branch.get("properties", {})}
        merged_required = sorted(set(base_required) | set(branch.get("required", [])))
        flattened.append({"type": "object", "properties": merged_props, "required": merged_required})
    return {"title": schema.get("title"), "oneOf": flattened}


def method_to_pascal(method):
    tokens = re.split(r"[/_]", method)
    return "".join(t[:1].upper() + t[1:] for t in tokens if t)


# Known irregular method -> response-type name mappings the naming convention can't derive
# (see codex-app-server-protocol-v2/README.md for how these were discovered).
RESPONSE_OVERRIDES = {
    "account/read": "GetAccountResponse",
    "account/rateLimits/read": "GetAccountRateLimitsResponse",
    "account/usage/read": "GetAccountTokenUsageResponse",
    "account/workspaceMessages/read": "GetWorkspaceMessagesResponse",
    "account/login/start": "LoginAccountResponse",
    "account/sendAddCreditsNudgeEmail": "SendAddCreditsNudgeEmailResponse",
    "account/chatgptAuthTokens/refresh": "ChatgptAuthTokensRefreshResponse",
    "app/list": "AppsListResponse",
    "config/batchWrite": "ConfigWriteResponse",
    "config/value/write": "ConfigWriteResponse",
    "item/commandExecution/requestApproval": "CommandExecutionRequestApprovalResponse",
    "item/fileChange/requestApproval": "FileChangeRequestApprovalResponse",
    "item/permissions/requestApproval": "PermissionsRequestApprovalResponse",
    "item/tool/call": "DynamicToolCallResponse",
    "item/tool/requestUserInput": "ToolRequestUserInputResponse",
    "mcpServer/resource/read": "McpResourceReadResponse",
    "remoteControl/client/list": "RemoteControlClientsListResponse",
    "remoteControl/client/revoke": "RemoteControlClientsRevokeResponse",
    # config/mcpServer/reload has no response payload (params/result are both `undefined`)
}


def camel_tokens(word):
    return [t.lower() for t in re.findall(r"[A-Z]?[a-z0-9]+|[A-Z]+(?=[A-Z]|$)", word) if t]


def method_to_snake_fn(method):
    tokens = []
    for segment in method.split("/"):
        tokens.extend(camel_tokens(segment))
    return "_".join(tokens)


def fuzzy_response_match(method, all_defs):
    method_tokens = set()
    for tok in re.split(r"[/_]", method):
        method_tokens.update(camel_tokens(tok))
    candidates = []
    for name in all_defs:
        if not name.endswith("Response"):
            continue
        base_tokens = set(camel_tokens(name[: -len("Response")]))
        if method_tokens <= base_tokens:
            candidates.append(name)
    candidates.sort(key=len)
    return candidates[0] if candidates else None


def methods_of(union_def):
    return [entry["properties"]["method"]["enum"][0] for entry in union_def["oneOf"]]


def build_methods_manifest(combined_defs):
    client_methods = methods_of(combined_defs["ClientRequest"])
    server_methods = methods_of(combined_defs["ServerRequest"])
    notif_methods = methods_of(combined_defs["ServerNotification"])
    client_notif_methods = methods_of(combined_defs["ClientNotification"])

    # Methods confirmed (by hand, against the schema) to genuinely have no
    # response payload / no params - NOT "our heuristic failed to find one."
    # Anything else the heuristics can't resolve is a hard build-time error,
    # so a future codex schema change that introduces a new shape breaks the
    # build loudly instead of silently generating wrong code (dropped params,
    # dropped response data). See README.md "Regenerating the schema".
    KNOWN_VOID_RESPONSE_METHODS = {"config/mcpServer/reload"}

    def params_type_for(method, union_def):
        """Returns (type_name, optional) for a plain or nullable $ref params
        schema, or (None, False) when the method genuinely has no params
        (either no "params" property at all - e.g. ClientNotification's
        "initialized" - or an explicit `"params": {"type": "null"}`, matching
        a `params: ()` unit field in the generated Rust). Raises if the params
        schema is some other shape (inline object, 3+-way anyOf, oneOf, ...)
        we haven't taught this function to recognize - that's a build-time
        bug, not something to silently paper over as "no params."""
        for entry in union_def["oneOf"]:
            if entry["properties"]["method"]["enum"][0] != method:
                continue
            params_schema = entry["properties"].get("params")
            if params_schema is None or params_schema.get("type") == "null":
                return None, False
            if "$ref" in params_schema:
                return params_schema["$ref"].rsplit("/", 1)[-1], False
            any_of = params_schema.get("anyOf")
            if any_of and len(any_of) == 2:
                refs = [b for b in any_of if "$ref" in b]
                nulls = [b for b in any_of if b.get("type") == "null"]
                if len(refs) == 1 and len(nulls) == 1:
                    return refs[0]["$ref"].rsplit("/", 1)[-1], True
            raise ValueError(
                f"{method}: unrecognized 'params' schema shape {params_schema!r} - not a plain "
                "$ref, a nullable $ref, or an explicit null. Update params_type_for to handle "
                "this shape (or the wrapper codegen in build.rs will silently emit `params: ()` "
                "for a method that actually requires typed params)."
            )
        raise ValueError(f"{method}: not found in the given union's oneOf branches")

    def resolve_response(method):
        candidate = RESPONSE_OVERRIDES.get(method) or (method_to_pascal(method) + "Response")
        if candidate in combined_defs:
            return candidate
        fuzzy = fuzzy_response_match(method, combined_defs)
        if fuzzy:
            return fuzzy
        if method in KNOWN_VOID_RESPONSE_METHODS:
            return None
        raise ValueError(
            f"{method}: could not resolve a response type (checked RESPONSE_OVERRIDES, the "
            f"{method_to_pascal(method)}Response naming convention, and a fuzzy token-subset "
            "match) and it is not in KNOWN_VOID_RESPONSE_METHODS. Either add an override, add a "
            "fuzzy-matchable response type, or - only if you've confirmed by hand that this "
            "method truly returns no payload - add it to KNOWN_VOID_RESPONSE_METHODS."
        )

    manifest = {"client_requests": [], "server_requests": [], "server_notifications": [], "client_notifications": []}
    for m in client_methods:
        ptype, popt = params_type_for(m, combined_defs["ClientRequest"])
        manifest["client_requests"].append({
            "method": m, "variant_name": method_to_pascal(m), "fn_name": method_to_snake_fn(m),
            "params_type": ptype, "params_optional": popt, "response_type": resolve_response(m),
        })
    for m in server_methods:
        ptype, popt = params_type_for(m, combined_defs["ServerRequest"])
        manifest["server_requests"].append({
            "method": m, "variant_name": method_to_pascal(m), "fn_name": method_to_snake_fn(m),
            "params_type": ptype, "params_optional": popt, "response_type": resolve_response(m),
        })
    for m in notif_methods:
        ptype, popt = params_type_for(m, combined_defs["ServerNotification"])
        manifest["server_notifications"].append({
            "method": m, "variant_name": method_to_pascal(m), "fn_name": method_to_snake_fn(m),
            "params_type": ptype, "params_optional": popt,
        })
    for m in client_notif_methods:
        ptype, popt = params_type_for(m, combined_defs["ClientNotification"])
        manifest["client_notifications"].append({
            "method": m, "variant_name": method_to_pascal(m), "fn_name": method_to_snake_fn(m),
            "params_type": ptype, "params_optional": popt,
        })
    return manifest


def main():
    master = json.load(open(GEN_DIR / "codex_app_server_protocol.schemas.json"))
    v2 = json.load(open(GEN_DIR / "codex_app_server_protocol.v2.schemas.json"))

    master_flat = {k: v for k, v in master["definitions"].items() if k != "v2"}
    master_flat_rewritten = {k: rewrite_v2_refs(v) for k, v in master_flat.items()}

    combined_defs = {**master_flat_rewritten, **v2["definitions"]}  # v2 wins name collisions

    # known typify-0.7.0 limitation workaround (see module docstring)
    if "McpServerElicitationRequestParams" in combined_defs:
        combined_defs["McpServerElicitationRequestParams"] = flatten_base_plus_oneof(
            combined_defs["McpServerElicitationRequestParams"]
        )

    combined = {
        "$schema": "http://json-schema.org/draft-07/schema#",
        "title": "CodexAppServerProtocolCombined",
        "description": (
            "Merged, flat-ref, self-contained v2-only Codex app-server protocol schema "
            "(master envelope/ServerRequest/ClientNotification types ref-rewritten to flat "
            "+ v2 client-request/notification surface). Generated by build_combined_schema.py."
        ),
        "type": "object",
        "definitions": combined_defs,
    }

    OUT_PATH.write_text(json.dumps(combined, indent=2))

    remaining_v2_refs = re.findall(r"#/definitions/v2/\w+", json.dumps(combined))
    print(f"total definitions: {len(combined_defs)}", file=sys.stderr)
    print(f"remaining v2-prefixed refs (must be 0): {len(remaining_v2_refs)}", file=sys.stderr)
    assert not remaining_v2_refs, "ref rewrite incomplete"
    print(f"wrote {OUT_PATH}", file=sys.stderr)

    manifest = build_methods_manifest(combined_defs)
    methods_path = OUT_PATH.parent / "methods.json"
    methods_path.write_text(json.dumps(manifest, indent=2))
    missing_response = [
        e["method"] for e in manifest["client_requests"] + manifest["server_requests"] if e["response_type"] is None
    ]
    print(f"client_requests={len(manifest['client_requests'])} server_requests={len(manifest['server_requests'])} "
          f"server_notifications={len(manifest['server_notifications'])} "
          f"client_notifications={len(manifest['client_notifications'])}", file=sys.stderr)
    print(f"methods with no resolvable response type: {missing_response}", file=sys.stderr)
    print(f"wrote {methods_path}", file=sys.stderr)


if __name__ == "__main__":
    main()

import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { ACTIONS, normalizeApiBaseUrl, REST_ACTIONS } from "./soma";

type OpenApiActionMetadata = {
  components: {
    schemas: {
      ActionName: {
        enum: string[];
      };
    };
  };
  "x-soma": {
    rest_actions: string[];
    mcp_only_actions: string[];
    direct_rest_routes: Record<string, { method: "GET" | "POST"; path: string }>;
  };
};

const here = dirname(fileURLToPath(import.meta.url));
const openApi = JSON.parse(
  readFileSync(resolve(here, "../../../docs/generated/openapi.json"), "utf8"),
) as OpenApiActionMetadata;

describe("Soma action metadata", () => {
  it("keeps REST actions aligned with generated OpenAPI metadata", () => {
    const webRestActions = REST_ACTIONS.map((action) => action.id);
    expect(webRestActions).toEqual(openApi.components.schemas.ActionName.enum);
    expect(webRestActions).toEqual(openApi["x-soma"].rest_actions);
  });

  it("keeps direct REST route metadata aligned with generated OpenAPI metadata", () => {
    const webRoutes = Object.fromEntries(
      REST_ACTIONS.map((action) => [
        action.id,
        {
          method: action.method,
          path: action.path,
        },
      ]),
    );
    expect(webRoutes).toEqual(openApi["x-soma"].direct_rest_routes);
  });

  it("keeps MCP-only actions aligned with generated OpenAPI metadata", () => {
    const webMcpOnlyActions = ACTIONS.filter((action) => action.transport === "mcp-only").map(
      (action) => action.id,
    );
    expect(webMcpOnlyActions).toEqual(openApi["x-soma"].mcp_only_actions);
  });

  it("does not duplicate action identifiers", () => {
    const ids = ACTIONS.map((action) => action.id);
    expect(new Set(ids).size).toBe(ids.length);
  });
});

describe("normalizeApiBaseUrl", () => {
  it("removes one or more trailing slashes", () => {
    expect(normalizeApiBaseUrl("http://localhost:40060/")).toBe("http://localhost:40060");
    expect(normalizeApiBaseUrl("http://localhost:40060///")).toBe("http://localhost:40060");
  });

  it("preserves empty same-origin configuration", () => {
    expect(normalizeApiBaseUrl("")).toBe("");
  });
});

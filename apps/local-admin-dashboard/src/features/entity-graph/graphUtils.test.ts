import { describe, expect, it } from "vitest";
import type { GraphNode } from "./types";
import { entityRoute, labelForMode, toneForStatus } from "./graphUtils";

const baseNode: GraphNode = {
  id: "resource:https://api.example.test/customers",
  type: "resource",
  entity_id: "https://api.example.test/customers",
  label: "Customers API",
  subtitle: "api",
  status: "observed",
  risk: "confidential",
  mode: "observe",
  badges: [],
  metrics: [],
};

describe("graphUtils", () => {
  it("builds selected routes for entity ids with reserved URL characters", () => {
    const route = entityRoute(baseNode);

    expect(route).toBe(
      "/resources?selected=https%3A%2F%2Fapi.example.test%2Fcustomers",
    );
  });

  it("honors backend hrefs when the graph read model provides one", () => {
    const route = entityRoute({
      ...baseNode,
      href: "/resources?selected=resource%2Ffrom%2Fbackend",
    });

    expect(route).toBe("/resources?selected=resource%2Ffrom%2Fbackend");
  });

  it("normalizes status and enforcement mode labels for cards", () => {
    expect(toneForStatus("restricted")).toBe("danger");
    expect(toneForStatus("Enforce")).toBe("success");
    expect(labelForMode("approval")).toBe("Approval");
  });
});

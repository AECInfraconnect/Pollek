import { test, expect } from "@playwright/test";
import { installMockApi } from "./mock-api";

test.describe("Policy-First Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await installMockApi(page);
    await page.addInitScript(() => {
      localStorage.setItem("pollek.mode", "desktop_advanced");
    });
    await page.goto("/");
  });

  test("should render sidebar and navigate to simple sections", async ({
    page,
  }) => {
    // 1. Dashboard Overview
    await expect(
      page.getByRole("heading", { name: "Dashboard Overview" }),
    ).toBeVisible();

    // 2. Find AI Apps / legacy Scan & Discover
    await page
      .getByRole("link", { name: /(find ai apps|scan & discover)/i })
      .click();
    await expect(
      page.getByRole("heading", { name: "Auto Discovery" }),
    ).toBeVisible();

    // 3. Activity
    await page.getByRole("link", { name: /^AI Activity$/i }).click();
    await expect(
      page
        .getByRole("heading", { name: /(activity|กิจกรรม)/i, exact: false })
        .first(),
    ).toBeVisible();

    // 4. Prompt Guard / alerts
    await page.getByRole("link", { name: "Prompt Guard", exact: true }).click();
    await expect(
      page.getByRole("heading", {
        name: "Prompt Guard, alerts, and Shadow AI",
      }),
    ).toBeVisible();
  });

  test("relationship and activity pages do not show raw Vite fallback HTML", async ({
    page,
  }) => {
    await page.goto("/entity-graph");
    await expect(
      page.getByRole("heading", { name: "Relationship Map" }),
    ).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);

    await page.goto("/activity-timeline");
    await expect(
      page.getByRole("heading", { name: "Activity Timeline" }),
    ).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);
  });

  test("API HTML fallback errors are shortened for operators", async ({
    page,
  }) => {
    await page.route("**/v1/tenants/local/entity-graph**", (route) =>
      route.fulfill({
        status: 200,
        contentType: "text/html",
        body: '<!doctype html><html><head><script type="module" src="/assets/index.js"></script></head></html>',
      }),
    );

    await page.goto("/entity-graph");

    await expect(
      page.getByText(
        "Local Control Plane API returned dashboard HTML instead of JSON",
      ),
    ).toBeVisible();
    await expect(page.getByText('script type="module"')).toHaveCount(0);
  });
});

test.describe("Simple mode wording guard", () => {
  test.skip(
    process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === "1",
    "Mock wording guard runs in dashboard-ci; the external-server job focuses on real Local Control Plane integration.",
  );

  test.beforeEach(async ({ page }) => {
    await installMockApi(page);
    await page.addInitScript(() => {
      localStorage.setItem("pollek.mode", "desktop_simple");
    });
  });

  async function expectNormalUserCopy(pageText: string) {
    expect(pageText).not.toMatch(/\b(PEP|PDP|WFP|eBPF|NetworkExtension)\b/);
    expect(pageText.toLowerCase()).not.toContain("control plane");
    expect(pageText).not.toMatch(
      /โ[\u0080-\u00ff]|โ€|�|Â|à|เธ[\u0080-\u00ff]|เน[\u0080-\u00ff]/,
    );
  }

  test("normal-user pages hide technical jargon and mojibake", async ({
    page,
  }) => {
    await page.goto("/scan");
    await page.getByRole("button", { name: /^Deep Scan$/ }).first().click();
    await expect(page.getByText("Antigravity").first()).toBeVisible({
      timeout: 20_000,
    });
    await expectNormalUserCopy(await page.locator("body").innerText());

    await page.goto("/activity");
    await expect(page.getByRole("heading", { name: "AI Activity" })).toBeVisible();
    await expectNormalUserCopy(await page.locator("body").innerText());

    await page.goto("/protect?agent_id=agent-antigravity&target=repo%2Fsrc&event=evt-governance-loop-1&intent=block_folder_access");
    await expect(
      page.getByRole("heading", { name: /Create AI Activity Rule/i }),
    ).toBeVisible();
    await expectNormalUserCopy(await page.locator("body").innerText());
  });
});

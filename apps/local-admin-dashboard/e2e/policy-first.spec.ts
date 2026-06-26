import { test, expect } from "@playwright/test";
import { installMockApi } from "./mock-api";

test.describe("Policy-First Navigation", () => {
  test.beforeEach(async ({ page }) => {
    await installMockApi(page);
    await page.goto("/");
  });

  test("should render sidebar and navigate to simple sections", async ({ page }) => {
    // 1. Dashboard Overview
    await expect(page.getByRole("heading", { name: "Dashboard Overview" })).toBeVisible();

    // 2. Protect
    await page.getByRole("link", { name: /(protect|สแกน)/i }).click();
    await expect(page.getByRole("heading", { name: /(protect|สแกน)/i, exact: false }).first()).toBeVisible();

    // 3. Activity
    await page.getByRole("link", { name: /(activity|กิจกรรม)/i }).click();
    await expect(page.getByRole("heading", { name: /(activity|กิจกรรม)/i, exact: false }).first()).toBeVisible();

    // 4. Alerts
    await page.getByRole("link", { name: /(alerts|แจ้งเตือน)/i }).click();
    await expect(page.getByRole("heading", { name: /(alerts|แจ้งเตือน)/i, exact: false }).first()).toBeVisible();
  });

  test("relationship and activity pages do not show raw Vite fallback HTML", async ({ page }) => {
    await page.goto("/entity-graph");
    await expect(page.getByRole("heading", { name: "Relationship Map" })).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);

    await page.goto("/activity-timeline");
    await expect(page.getByRole("heading", { name: "Activity Timeline" })).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);
  });

  test("API HTML fallback errors are shortened for operators", async ({ page }) => {
    await page.route("**/v1/tenants/local/entity-graph", (route) =>
      route.fulfill({
        status: 200,
        contentType: "text/html",
        body: '<!doctype html><html><head><script type="module" src="/assets/index.js"></script></head></html>',
      }),
    );

    await page.goto("/entity-graph");

    await expect(
      page.getByText("Local Control Plane API returned dashboard HTML instead of JSON"),
    ).toBeVisible();
    await expect(page.getByText('script type="module"')).toHaveCount(0);
  });
});

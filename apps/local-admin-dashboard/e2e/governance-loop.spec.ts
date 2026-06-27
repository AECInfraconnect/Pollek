import { expect, test } from "@playwright/test";
import { installMockApi } from "./mock-api";

test.describe("Governance loop", () => {
  test.beforeEach(async ({ page }) => {
    await installMockApi(page);
    await page.addInitScript(() => {
      localStorage.setItem("pollek.mode", "desktop_advanced");
    });
  });

  test("scans, detects, suggests, and observes an enforced local agent flow", async ({
    page,
  }) => {
    await page.goto("/");
    await expect(
      page.getByRole("heading", { name: "Dashboard Overview" }),
    ).toBeVisible();
    await expect(page.getByTestId("current-device-label")).toBeVisible();
    await expect(page.getByTestId("current-device-label")).toHaveText(/\S/);
    await expect(
      page.getByRole("heading", { name: "Control Capabilities" }),
    ).toBeVisible();

    await page.goto("/scan");
    await expect(
      page.getByRole("heading", { name: "Auto Discovery" }),
    ).toBeVisible();
    await page.getByRole("button", { name: /^Deep Scan$/ }).first().click();
    await expect(page.getByText("Antigravity").first()).toBeVisible({
      timeout: 20_000,
    });
    await expect(page.getByText("workspace_file_access").first()).toBeVisible();

    await page.goto("/agents?id=agent-antigravity");
    await expect(page.getByText("Record Summary")).toBeVisible();
    await expect(page.getByRole("heading", { name: "Antigravity" })).toBeVisible();
    await page.getByRole("button", { name: /details/i }).click();
    await expect(
      page.getByRole("heading", { name: "Reference Intel" }),
    ).toBeVisible();
    await expect(
      page.getByRole("heading", { name: "Known Capability Checklist" }),
    ).toBeVisible();
    await expect(page.getByText("Workspace file access").first()).toBeVisible();
    await expect(page.getByText("Detected", { exact: true }).first()).toBeVisible();
    await expect(page.getByText("Source: Auto Discovery")).toBeVisible();

    await page.goto("/policy-suggestions");
    await page.getByRole("button", { name: /generate suggestions/i }).click();
    await expect(page.getByText("Protect workspace source files")).toBeVisible();
    await expect(page.getByRole("link", { name: /deploy policy/i })).toBeVisible();

    await page.goto("/policies?id=policy-protect-workspace-files");
    await expect(
      page.getByRole("heading", { name: "Protect workspace source files" }),
    ).toBeVisible();
    await page.getByRole("button", { name: /details/i }).click();
    await expect(page.getByText("Deployment & History")).toBeVisible();
    await expect(page.getByText("bundle-local-1")).toBeVisible();

    await page.goto("/activity-timeline");
    await expect(
      page.getByRole("heading", { name: "Activity Timeline" }),
    ).toBeVisible();
    await expect(page.getByText("filesystem.read")).toBeVisible();
    await expect(page.getByText("Protect workspace source files")).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);
  });
});

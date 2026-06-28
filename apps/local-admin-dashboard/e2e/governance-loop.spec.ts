import { expect, test } from "@playwright/test";
import { installMockApi } from "./mock-api";

test.describe("Governance loop", () => {
  test.skip(
    process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === "1",
    "Runs in the dashboard mock E2E job; the external-server job validates the real Local Control Plane.",
  );

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
    await expect(page.getByText("Browser Control").first()).toBeVisible();

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
    await page.getByRole("button", { name: /observe coverage/i }).click();
    const observeCoverage = page.getByTestId("agent-observe-coverage");
    await expect(observeCoverage).toBeVisible();
    await expect(
      observeCoverage.getByText("What Pollek can see for this AI app"),
    ).toBeVisible();
    const filesCoverage = observeCoverage.getByTestId(
      "agent-observe-coverage-files",
    );
    const costCoverage = observeCoverage.getByTestId(
      "agent-observe-coverage-cost",
    );
    await expect(filesCoverage).toBeVisible();
    await expect(filesCoverage).toContainText("Files and folders");
    await expect(costCoverage).toBeVisible();
    await expect(costCoverage).toContainText("AI usage and cost");

    await page.goto("/agents");
    const agentCard = page.getByRole("option", { name: /Antigravity/ }).first();
    await expect(agentCard).toHaveCSS("cursor", "pointer");
    await expect(
      agentCard.getByRole("button", { name: /show more/i }),
    ).toBeVisible();
    await agentCard.getByRole("button", { name: /show more/i }).click();
    await expect(page.getByText("Observed process")).toBeVisible();

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
    await expect(
      page.getByText("Antigravity used Workspace Files on repo/src"),
    ).toBeVisible();
    await expect(page.getByText("Protect workspace source files")).toBeVisible();
    await page
      .getByText("Antigravity used Workspace Files on repo/src")
      .first()
      .click();
    await expect(page.getByText("Back to all timeline events")).toBeVisible();
    await expect(page.getByText("Detail Workspace")).toBeVisible();
    await expect(
      page.getByRole("link", { name: /create rule from event/i }),
    ).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);
  });
});

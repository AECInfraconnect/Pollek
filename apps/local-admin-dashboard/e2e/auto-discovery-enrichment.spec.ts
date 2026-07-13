import { expect, test } from "@playwright/test";
import { installMockApi } from "./mock-api";

function installPageErrorGuard(page: import("@playwright/test").Page) {
  const pageErrors: string[] = [];
  page.on("pageerror", (error) => {
    pageErrors.push(`${page.url()}: ${error.message}`);
  });
  page.on("console", (message) => {
    if (
      message.type() === "error" &&
      /Objects are not valid as a React child/.test(message.text())
    ) {
      pageErrors.push(`${page.url()}: ${message.text()}`);
    }
  });
  return pageErrors;
}

test.describe("Auto Discovery grouped enrichment", () => {
  test.setTimeout(90_000);

  test.skip(
    process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === "1",
    "Uses mock API fixtures to validate interactive discovery UI states.",
  );

  test.beforeEach(async ({ page }) => {
    await installMockApi(page);
    await page.addInitScript(() => {
      localStorage.setItem("pollek.mode", "desktop_advanced");
      localStorage.setItem("pollek.theme", "dark");
    });
  });

  test("shows real scan state, per-scan grouping, grouped child surfaces, and enrichment loop", async ({
    page,
  }) => {
    const pageErrors = installPageErrorGuard(page);
    await page.setViewportSize({ width: 1440, height: 900 });
    await page.goto("/scan");

    await expect(page.getByRole("heading", { name: "Auto Discovery" })).toBeVisible();

    const coverageToggle = page.getByRole("button", {
      name: /Scan source coverage/i,
    });
    await expect(coverageToggle).toBeVisible();
    await expect(
      page.getByText("Privacy guardrail: discovery uses metadata"),
    ).toBeHidden();

    const deepScanButton = page.getByRole("button", { name: /^Deep Scan$/ }).first();
    await deepScanButton.click();
    await expect(
      page.getByRole("button", { name: /Scanning|Updating Results/i }).first(),
    ).toBeVisible();

    await expect(page.getByText(/Latest Scan -/).first()).toBeVisible({
      timeout: 20_000,
    });
    await expect(
      page.getByRole("option", { name: /Antigravity/ }).first(),
    ).toBeVisible();
    const aiStudioCard = page
      .getByRole("option", { name: /Google AI Studio/ })
      .first();
    await expect(aiStudioCard).toBeVisible();
    await expect(aiStudioCard).toHaveCSS("cursor", "pointer");
    await expect(aiStudioCard).toContainText("Controlled through parent");

    await aiStudioCard.click();
    await expect(page.getByText("Detail Workspace")).toBeVisible();
    await expect(page.getByText("Identity and grouping")).toBeVisible();
    await expect(page.getByText("Controlled through parent").first()).toBeVisible();
    await expect(page.getByText("Google AI Studio is related").first()).toBeVisible();

    await page.getByRole("button", { name: /^Enrich$/ }).click();
    await expect(page.getByText("Privacy guardrails")).toBeVisible();
    await expect(page.getByText("Official product documentation")).toBeVisible();

    await page.getByRole("button", { name: "Approve safe sources" }).click();
    await expect(page.getByText("Enrichment result")).toBeVisible();
    await expect(page.getByText("browser-scoped child surface")).toBeVisible();

    await page.getByRole("button", { name: "Save local profile" }).click();
    await expect(page.getByText("Local learned profile saved")).toBeVisible();
    expect(pageErrors).toEqual([]);
  });
});

import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { installMockApi } from "./mock-api";

const entityPages = [
  {
    path: "/agents",
    masterHeading: "Agents & Models",
    detailText: "Detail Workspace",
  },
  {
    path: "/activity-timeline",
    masterHeading: "Activity Timeline",
    detailText: "Create rule from event",
  },
  {
    path: "/deployments",
    masterHeading: "Deployments",
    detailText: "Rollback",
  },
] as const;

async function seedObservedAgent(page: Page) {
  await page.goto("/scan");
  await page.getByRole("button", { name: /^Deep Scan$/ }).first().click();
  await expect(page.getByText("Antigravity").first()).toBeVisible({
    timeout: 20_000,
  });
}

test.describe("Visual smoke for entity pages", () => {
  test.skip(
    process.env.DEK_PLAYWRIGHT_EXTERNAL_SERVER === "1",
    "Uses mock API fixtures to validate dashboard layout states.",
  );

  test.beforeEach(async ({ page }) => {
    await installMockApi(page);
    await page.addInitScript(() => {
      localStorage.setItem("pollek.mode", "desktop_advanced");
    });
  });

  for (const pageCase of entityPages) {
    test(`${pageCase.masterHeading} has usable master/detail layout in desktop and mobile`, async ({
      page,
    }) => {
      await page.setViewportSize({ width: 1440, height: 900 });
      await seedObservedAgent(page);
      await page.goto(pageCase.path);
      await expect(
        page.getByRole("heading", { name: pageCase.masterHeading }),
      ).toBeVisible();

      const masterList = page.getByRole("listbox", { name: "Items" });
      const firstCard = masterList.getByRole("option").first();
      await expect(firstCard).toBeVisible();
      await expect(firstCard).toHaveCSS("cursor", "pointer");

      await firstCard.click();
      await expect(page.getByText(pageCase.detailText).first()).toBeVisible();
      await expect(page.getByText("<!doctype html")).toHaveCount(0);

      await page.setViewportSize({ width: 390, height: 844 });
      await page.goto(pageCase.path);
      await expect(
        page.getByRole("heading", { name: pageCase.masterHeading }),
      ).toBeVisible();
      await expect(
        page
          .getByRole("listbox", { name: "Items" })
          .getByRole("option")
          .first(),
      ).toBeVisible();
      await expect(page.getByText("<!doctype html")).toHaveCount(0);
    });
  }

  test("light mode keeps core dashboard text readable", async ({ page }) => {
    await page.setViewportSize({ width: 1280, height: 800 });
    await seedObservedAgent(page);
    await page.goto("/agents");
    await page.getByRole("button", { name: "Switch to light mode" }).click();

    await expect(page.locator("html")).not.toHaveClass(/dark/);
    await expect(
      page.getByRole("heading", { name: "Agents & Models" }),
    ).toBeVisible();
    await expect(
      page
        .getByRole("listbox", { name: "Items" })
        .getByRole("option", { name: /Antigravity/ }),
    ).toBeVisible();
    await expect(page.getByText("<!doctype html")).toHaveCount(0);
  });
});

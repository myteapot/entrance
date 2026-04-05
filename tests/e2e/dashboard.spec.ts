import { expect, test } from "@playwright/test";
import { installTauriMocks } from "./tauriHarness";

test.beforeEach(async ({ page }) => {
  await installTauriMocks(page);
});

test("renders the dashboard map region", async ({ page }) => {
  await page.goto("/board");

  await expect(page.locator(".board-map")).toBeVisible();
  await expect(page.locator(".board-map__grid")).toBeVisible();
});

test("renders the dashboard hero pills", async ({ page }) => {
  await page.goto("/board");

  const hero = page.locator(".page__hero--dashboard");

  await expect(hero).toBeVisible();
  expect(await hero.locator(".dashboard-pill").count()).toBeGreaterThan(0);
});

test("renders the dashboard detail cards", async ({ page }) => {
  await page.goto("/board");

  expect(await page.locator(".dashboard-card").count()).toBeGreaterThanOrEqual(4);
});

import { expect, test, type Page } from "@playwright/test";
import { installTauriMocks } from "./tauriHarness";

const sidebarLink = (page: Page, label: string) =>
  page.locator(".sidebar__link", {
    has: page.locator(".sidebar__label", { hasText: new RegExp(`^${label}$`) }),
  });

test.beforeEach(async ({ page }) => {
  await installTauriMocks(page);
});

test("loads the shell and sidebar navigation", async ({ page }) => {
  const response = await page.goto("/");

  expect(response?.status()).toBe(200);
  await expect(page.locator(".app-shell")).toBeVisible();
  await expect(page.locator(".sidebar__link")).toHaveCount(6);
  await expect(sidebarLink(page, "Chat")).toBeVisible();
  await expect(sidebarLink(page, "Do")).toBeVisible();
  await expect(sidebarLink(page, "Board")).toBeVisible();
  await expect(sidebarLink(page, "Console")).toBeVisible();
  await expect(sidebarLink(page, "Issues")).toBeVisible();
  await expect(sidebarLink(page, "Settings")).toBeVisible();
});

test("switches routes from the sidebar", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveURL(/\/$/);
  await expect(page.locator(".page--chat")).toBeVisible();
  await expect(page.locator(".page__eyebrow", { hasText: /^Chat$/ })).toBeVisible();

  await sidebarLink(page, "Do").click();
  await expect(page).toHaveURL(/\/do$/);
  await expect(page.getByRole("heading", { name: "Do", exact: true })).toBeVisible();

  await sidebarLink(page, "Board").click();
  await expect(page).toHaveURL(/\/board$/);
  await expect(page.locator(".page--dashboard")).toBeVisible();
  await expect(page.locator(".page__eyebrow", { hasText: /^Board$/ })).toBeVisible();

  await sidebarLink(page, "Issues").click();
  await expect(page).toHaveURL(/\/issues$/);
  await expect(page.getByRole("heading", { name: "Issues", exact: true })).toBeVisible();

  await sidebarLink(page, "Settings").click();
  await expect(page).toHaveURL(/\/settings$/);
  await expect(page.getByRole("heading", { name: "Vault" })).toBeVisible();
});

test("shows the dashboard route and mission board shell", async ({ page }) => {
  await page.goto("/board");

  await expect(page.locator(".page--dashboard")).toBeVisible();
  await expect(page.locator(".page__eyebrow", { hasText: /^Board$/ })).toBeVisible();
  await expect(page.locator(".board-map")).toBeVisible();
});

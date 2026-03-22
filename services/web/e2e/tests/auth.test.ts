import { expect, test } from "@playwright/test";

test.describe("Authentication flow", () => {
  test("home page renders with sign-in and sign-up links", async ({ page }) => {
    await page.goto("/");
    await expect(page.getByRole("heading", { level: 1 })).toBeVisible();
    await expect(page.getByRole("link", { name: /get started/i }))
      .toHaveAttribute("href", "/sign-up");
    await expect(page.getByRole("link", { name: /sign in/i })).toHaveAttribute(
      "href",
      "/login",
    );
  });

  test("login page renders form with email and password fields", async ({ page }) => {
    await page.goto("/login");
    await expect(page.getByLabel(/email/i)).toBeVisible();
    await expect(page.getByLabel(/password/i)).toBeVisible();
    await expect(page.getByRole("button", { name: /sign in/i })).toBeVisible();
  });

  test("sign-up page renders form with username, email, and password fields", async ({ page }) => {
    await page.goto("/sign-up");
    await expect(page.getByLabel(/username/i)).toBeVisible();
    await expect(page.getByLabel(/email/i)).toBeVisible();
    await expect(page.getByLabel(/password/i)).toBeVisible();
    await expect(page.getByRole("button", { name: /create account/i }))
      .toBeVisible();
  });

  test("login page links to sign-up", async ({ page }) => {
    await page.goto("/login");
    const signUpLink = page.getByRole("link", { name: /sign up/i });
    await expect(signUpLink).toHaveAttribute("href", "/sign-up");
  });

  test("sign-up page links to login", async ({ page }) => {
    await page.goto("/sign-up");
    const signInLink = page.getByRole("link", { name: /sign in/i });
    await expect(signInLink).toHaveAttribute("href", "/login");
  });

  test("404 page renders for unknown routes", async ({ page }) => {
    await page.goto("/nonexistent");
    await expect(page.getByText("404")).toBeVisible();
    await expect(page.getByRole("link", { name: /go home/i })).toHaveAttribute(
      "href",
      "/",
    );
  });
});

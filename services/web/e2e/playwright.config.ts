import { defineConfig, devices } from "@playwright/test";

const baseURL = Deno.env.get("WEB_BASE_URL") ?? "http://localhost:3000";

export default defineConfig({
  testDir: "./tests",
  fullyParallel: true,
  forbidOnly: !!Deno.env.get("CI"),
  retries: Deno.env.get("CI") ? 2 : 0,
  workers: Deno.env.get("CI") ? 1 : undefined,
  reporter: "html",
  use: {
    baseURL,
    trace: "on-first-retry",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chrome"] },
    },
  ],
});

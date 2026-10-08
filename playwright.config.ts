import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/browser",
  timeout: 120000,
  expect: { timeout: 15000 },
  workers: 1,
  use: {
    channel: "chromium",
    baseURL: "http://127.0.0.1:5173",
    viewport: { width: 1440, height: 1000 },
    launchOptions: {
      args: [
        process.platform === "darwin" ? "--use-angle=metal" : "--use-angle=swiftshader",
        "--enable-webgl",
      ],
    },
    screenshot: "only-on-failure",
  },
  webServer: [
    {
      command: "LLM_PROVIDER=mock bun server/index.ts",
      url: "http://127.0.0.1:3001/api/status",
      reuseExistingServer: false,
    },
    {
      command: "bun x vite --host 127.0.0.1",
      url: "http://127.0.0.1:5173",
      reuseExistingServer: false,
    },
  ],
});

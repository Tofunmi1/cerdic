import { expect, test } from "@playwright/test";
import { PerspectiveCamera, Vector3 } from "three";

test("chat composition, stable follow-up IDs, manual edits, history and persistence", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  await expect(page.getByText("Mock mode · no LLM connected")).toBeVisible();
  await expect(page.locator("canvas")).toBeVisible();
  await page.screenshot({ path: "test-results/workspace-initial.png" });
  await page.getByRole("button", { name: "Build a simple table" }).click();
  await expect(page.locator(".scene-row")).toHaveCount(9);
  await expect(page.getByRole("textbox", { name: "Object name" })).toHaveValue(
    "Table top",
  );
  const stableId = await page.locator(".object-heading code").textContent();
  await page
    .getByRole("textbox", { name: "Describe your scene" })
    .fill("Move it left 2 and make it blue");
  await page.getByRole("button", { name: "Send prompt" }).click();
  await expect(
    page.getByRole("spinbutton", { name: "position X", exact: true }),
  ).toHaveValue("-2");
  await expect(page.getByRole("textbox", { name: "Hex colour" })).toHaveValue(
    "#8ca8df",
  );
  expect(await page.locator(".object-heading code").textContent()).toBe(
    stableId,
  );
  const name = page.getByRole("textbox", { name: "Object name" });
  await name.fill("My table");
  await name.press("Enter");
  await expect(
    page.getByRole("button", { name: "My table", exact: true }),
  ).toBeVisible();
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(name).toHaveValue("Table top");
  await page.getByRole("button", { name: "Redo", exact: true }).click();
  await expect(name).toHaveValue("My table");
  const rotation = page.getByRole("spinbutton", {
    name: "rotation Y",
    exact: true,
  });
  await rotation.fill("45");
  await rotation.press("Enter");
  await expect(rotation).toHaveValue("45");
  const scale = page.getByRole("spinbutton", { name: "scale X", exact: true });
  await scale.fill("3");
  await scale.press("Enter");
  await expect(scale).toHaveValue("3");
  await page.getByRole("button", { name: "Rotate", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Rotate", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Scale", exact: true }).click();
  await expect(
    page.getByRole("button", { name: "Scale", exact: true }),
  ).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("status")).toContainText("Scene saved");
  await page.reload();
  await expect(page.locator(".scene-row")).toHaveCount(9);
  await page.getByRole("button", { name: "My table", exact: true }).click();
  await expect(
    page.getByRole("spinbutton", { name: "scale X", exact: true }),
  ).toHaveValue("3");
  expect(await page.locator(".object-heading code").textContent()).toBe(
    stableId,
  );
  await page.screenshot({ path: "test-results/workspace-desktop.png" });
  expect(errors).toEqual([]);
});

test("invalid saved data recovers, mobile fits and unsupported VR is disabled", async ({
  page,
}) => {
  await page.addInitScript(() =>
    localStorage.setItem("spatial-lab.scene.v2", "{broken"),
  );
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await expect(page.locator(".scene-row")).toHaveCount(4);
  await expect(page.getByRole("button", { name: "Enter VR" })).toBeDisabled();
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({
    path: "test-results/workspace-mobile.png",
    fullPage: true,
  });
});

test("API rejects invalid payloads without mutating scenes", async ({
  request,
}) => {
  const response = await request.post("/api/scene", {
    data: {
      prompt: "Add a box",
      scene: { version: 1, objects: [] },
      selectedId: null,
      apiKey: "not-allowed",
    },
  });
  expect(response.status()).toBe(400);
});

test("viewport selection and a gizmo drag commit one reversible move", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.locator("canvas")).toBeVisible();
  const camera = new PerspectiveCamera(40, 1440 / 1000, 0.1, 1000);
  camera.position.set(6, 4.8, 8);
  camera.lookAt(0, 0.75, 0);
  camera.updateMatrixWorld();
  const position = new Vector3(-0.75, 0.98, 0);
  const screen = (p: Vector3) => {
    const point = p.clone().project(camera);
    return { x: (point.x + 1) * 720, y: (1 - point.y) * 500 };
  };
  const center = screen(position);
  // Canvas visibility precedes the asynchronously created R3F event root.
  await expect(async () => {
    await page.mouse.click(center.x, center.y);
    await expect(
      page.getByRole("textbox", { name: "Object name" }),
    ).toHaveValue("Lavender block", { timeout: 1000 });
  }).toPass({ timeout: 15000 });
  await page.screenshot({ path: "test-results/gizmo-before.png" });
  const factor =
    (position.distanceTo(camera.position) *
      Math.min(1.9 * Math.tan((Math.PI * 40) / 360), 7) *
      0.75) /
    7;
  const handle = screen(position.clone().add(new Vector3(factor * 0.85, 0, 0)));
  await page.mouse.move(handle.x, handle.y);
  await page.mouse.down();
  await page.mouse.move(handle.x + 65, handle.y, { steps: 12 });
  await page.mouse.up();
  await page.screenshot({ path: "test-results/gizmo-after.png" });
  await expect(page.getByRole("textbox", { name: "Object name" })).toHaveValue(
    "Lavender block",
  );
  const x = page.getByRole("spinbutton", { name: "position X", exact: true });
  await expect(x).not.toHaveValue("-0.75");
  await page.getByRole("button", { name: "Undo", exact: true }).click();
  await expect(x).toHaveValue("-0.75");
  await expect(
    page.getByRole("button", { name: "Undo", exact: true }),
  ).toBeDisabled();
});

test("a delayed AI response cannot overwrite a newer manual edit", async ({
  page,
}) => {
  await page.goto("/");
  await expect(page.getByText("Mock mode · no LLM connected")).toBeVisible();
  await page.getByTestId("object-lavender-block").click();
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  await page.route("**/api/scene", async (route) => {
    await gate;
    await route.fulfill({
      json: {
        message: "Changed",
        commands: [
          { type: "update", id: "lavender-block", patch: { color: "#000000" } },
        ],
      },
    });
  });
  await page
    .getByRole("textbox", { name: "Describe your scene" })
    .fill("Make it black");
  await page.getByRole("button", { name: "Send prompt" }).click();
  await expect(page.getByText("Shaping your scene…")).toBeVisible();
  const name = page.getByRole("textbox", { name: "Object name" });
  await name.fill("Manual name");
  await name.press("Enter");
  release();
  await expect(
    page.getByText(
      "The scene changed while this request was running. Please send the prompt again.",
    ),
  ).toBeVisible();
  await expect(name).toHaveValue("Manual name");
  await expect(page.getByRole("textbox", { name: "Hex colour" })).toHaveValue(
    "#a99bd2",
  );
});

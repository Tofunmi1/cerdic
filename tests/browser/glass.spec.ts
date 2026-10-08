import { expect, test } from "@playwright/test";

test("controls use a lens, content stays standard, and interaction feedback works", async ({
  page,
}) => {
  const errors: string[] = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.goto("/");
  const toolbar = page.getByRole("toolbar", { name: "Scene tools" });
  await expect(toolbar).toHaveAttribute("data-optics", "lens");
  expect(
    await toolbar.evaluate(
      (el) => getComputedStyle(el, "::before").backdropFilter,
    ),
  ).toContain("url(");
  await expect(
    page.getByRole("region", { name: "Scene assistant" }),
  ).not.toHaveClass(/glass/);
  await toolbar.hover();
  expect(
    await toolbar.evaluate((el) =>
      (el as HTMLElement).style.getPropertyValue("--light-x"),
    ),
  ).not.toBe("");
  await page.getByRole("button", { name: "Rotate", exact: true }).click();
  expect(
    await page
      .locator(".tool-group")
      .evaluate((el) =>
        (el as HTMLElement).style.getPropertyValue("--tool-index"),
      ),
  ).toBe("1");
  await page.getByRole("button", { name: "Add object" }).click();
  await expect(page.locator(".add-menu")).toHaveAttribute(
    "data-optics",
    "lens",
  );
  await page.screenshot({ path: "test-results/liquid-glass-controls.png" });
  // A controlled background makes genuine backdrop displacement observable.
  await page.evaluate(() => {
    const proof = document.createElement("div");
    proof.className = "lens-proof";
    proof.style.cssText =
      "position:absolute;left:380px;top:180px;width:280px;height:100px;z-index:20;background:repeating-linear-gradient(90deg,#21416a 0 12px,#b7cae8 12px 24px);";
    const lens = document.createElement("div");
    lens.className = "glass";
    lens.style.cssText =
      "position:absolute;left:20px;top:18px;width:240px;height:64px;border-radius:32px;";
    proof.append(lens);
    document.querySelector(".workspace")!.append(proof);
  });
  const lens = page.locator(".lens-proof .glass");
  await expect(lens).toHaveAttribute("data-optics", "lens");
  const refracted = await page
    .locator(".lens-proof")
    .screenshot({ path: "test-results/glass-lens-proof.png" });
  await lens.evaluate((el) =>
    (el as HTMLElement).style.setProperty("--glass-filter", "blur(1.5px)"),
  );
  const flat = await page
    .locator(".lens-proof")
    .screenshot({ path: "test-results/glass-blur-baseline.png" });
  expect(refracted.equals(flat)).toBe(false);
  expect(errors).toEqual([]);
});

test("increased contrast and reduced motion remove optical and elastic effects", async ({
  page,
}) => {
  await page.emulateMedia({ contrast: "more", reducedMotion: "reduce" });
  await page.goto("/");
  const toolbar = page.getByRole("toolbar", { name: "Scene tools" });
  await expect(toolbar).toHaveAttribute("data-optics", "reduced");
  expect(
    await toolbar.evaluate(
      (el) => getComputedStyle(el, "::before").backdropFilter,
    ),
  ).toBe("none");
  expect(
    await page
      .locator(".tool-selection")
      .evaluate((el) => getComputedStyle(el).transitionDuration),
  ).toBe("0s");
  await page.screenshot({ path: "test-results/liquid-glass-accessible.png" });
});

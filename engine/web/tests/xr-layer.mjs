// The real-headset output path: the session layer owns an opaque, non-sRGB framebuffer (emulators
// do not, so this test makes one in JS and hands it out through the emulated layer). The engine must
// import it, render offscreen, and present into it with the sRGB encode done by hand. The pixels read
// back from that framebuffer must match the golden image rendered through the window path.
import fs from "node:fs";
import path from "node:path";
import { chromium } from "playwright";
import { PNG } from "pngjs";
import { differingFraction, driverAdvisory, GOLDEN, here, serve } from "./support.mjs";

const iwerBundle = fs.readFileSync(path.join(here, "..", "node_modules", "iwer", "build", "iwer.min.js"), "utf8");
const golden = PNG.sync.read(fs.readFileSync(path.join(here, "golden", "xr-640x360.png")));
const failures = [];
const check = (ok, message) => { if (!ok) failures.push(message); };

const { server, port } = await serve();
const browser = await chromium.launch({ args: ["--enable-unsafe-swiftshader", "--ignore-gpu-blocklist"] });
try {
  const page = await browser.newPage({ viewport: { width: 640, height: 360 } });
  const logs = [];
  page.on("console", (m) => {
    if (process.env.VERBOSE) console.log(`[${m.type()}] ${m.text().slice(0, 300)}`);
    if (["error", "warning"].includes(m.type()) && !driverAdvisory.test(m.text())) logs.push(`${m.type()}: ${m.text()}`);
  });
  page.on("pageerror", (e) => logs.push(`pageerror: ${e.message}`));
  await page.addInitScript({ content: iwerBundle });
  await page.addInitScript(() => {
    const device = new window.IWER.XRDevice(window.IWER.metaQuest3);
    device.installRuntime({ forceInstall: true });
    device.stereoEnabled = true;
    window.__xr = device;
    // Give the emulated layer a real framebuffer backed by a plain RGBA8 texture, like a headset's.
    Object.defineProperty(window.IWER.XRWebGLLayer.prototype, "framebuffer", {
      get() {
        const gl = this.context;
        if (!window.__layer) {
          const width = gl.drawingBufferWidth;
          const height = gl.drawingBufferHeight;
          const texture = gl.createTexture();
          gl.bindTexture(gl.TEXTURE_2D, texture);
          gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
          const framebuffer = gl.createFramebuffer();
          gl.bindFramebuffer(gl.FRAMEBUFFER, framebuffer);
          gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texture, 0);
          gl.bindFramebuffer(gl.FRAMEBUFFER, null);
          gl.bindTexture(gl.TEXTURE_2D, null);
          window.__layer = { gl, framebuffer, width, height };
        }
        return window.__layer.framebuffer;
      },
    });
    window.__readLayer = () => {
      const { gl, framebuffer, width, height } = window.__layer;
      const pixels = new Uint8Array(width * height * 4);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, framebuffer);
      gl.readPixels(0, 0, width, height, gl.RGBA, gl.UNSIGNED_BYTE, pixels);
      gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
      return { width, height, pixels: Array.from(pixels) };
    };
  });

  await page.goto(`http://127.0.0.1:${port}/?scene=xr&xr=auto`);
  await page.waitForFunction(() => window.__ve?.ready || window.__ve?.error, null, { timeout: 90000 });
  check(!(await page.evaluate(() => window.__ve.error)), "start failed");
  await page.waitForFunction(() => window.__ve.read().xr.state === "Presenting" && window.__ve.read().xr.frames >= 12, null, { timeout: 60000 });
  check(await page.evaluate(() => Boolean(window.__layer)), "the engine never asked the layer for its framebuffer");

  const { width, height, pixels } = await page.evaluate(() => window.__readLayer());
  check(width === 640 && height === 360, `layer framebuffer is ${width}x${height}`);
  // GL reads bottom-up; compare against the golden both ways up and keep the better match.
  const upright = new PNG({ width, height });
  const flipped = new PNG({ width, height });
  for (let y = 0; y < height; y++) {
    const row = Buffer.from(pixels.slice(y * width * 4, (y + 1) * width * 4));
    row.copy(flipped.data, (height - 1 - y) * width * 4);
    row.copy(upright.data, y * width * 4);
  }
  for (const image of [upright, flipped]) for (let i = 3; i < image.data.length; i += 4) image.data[i] = 255;
  if (process.env.DUMP) fs.writeFileSync(process.env.DUMP, PNG.sync.write(upright));
  const fractions = { asRead: differingFraction(upright, golden), flipped: differingFraction(flipped, golden) };
  console.log(JSON.stringify({ framebuffer: [width, height], differsFromGolden: fractions }));
  const best = Math.min(fractions.asRead, fractions.flipped);
  check(best <= GOLDEN.maxBadFraction, `the framebuffer differs from the golden image in ${(best * 100).toFixed(3)}% of pixels (as read ${(fractions.asRead * 100).toFixed(2)}%, flipped ${(fractions.flipped * 100).toFixed(2)}%)`);
  check(fractions.asRead <= fractions.flipped, "the picture is upside down in the framebuffer (GL origin handling is wrong)");

  const status = await page.evaluate(() => window.__ve.read());
  check(status.xr.error === null, `xr error: ${status.xr.error}`);
  check(logs.length === 0, `browser logged problems:\n  ${logs.join("\n  ")}`);
} finally {
  await browser.close();
  server.close();
}

if (failures.length > 0) {
  console.error(`\nFAILED (${failures.length}):\n- ${failures.join("\n- ")}`);
  process.exit(1);
}
console.log("\nxr layer checks passed");

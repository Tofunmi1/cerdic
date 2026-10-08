import { useEffect } from "react";
import type { RefObject } from "react";
import { makeLensMap } from "./glassOptics";

const svgNS = "http://www.w3.org/2000/svg";
let nextId = 0;

/** Progressive enhancement: original browser optics, not Apple's native material. */
export function useLiquidGlass(root: RefObject<HTMLElement | null>) {
  useEffect(() => {
    const container = root.current;
    if (!container) return;
    const reducedTransparency = matchMedia(
      "(prefers-reduced-transparency: reduce)",
    );
    const increasedContrast = matchMedia("(prefers-contrast: more)");
    const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
    // URL syntax alone is insufficient detection: Safari/Firefox may parse it
    // without rendering SVG backdrop displacement. Keep their blur fallback.
    const canLens =
      /Chrome|Chromium|Edg\//.test(navigator.userAgent) &&
      CSS.supports("backdrop-filter", 'url("#lens")');
    const svg = document.createElementNS(svgNS, "svg");
    svg.setAttribute("aria-hidden", "true");
    svg.setAttribute("width", "0");
    svg.setAttribute("height", "0");
    svg.classList.add("glass-definitions");
    const defs = document.createElementNS(svgNS, "defs");
    svg.append(defs);
    container.append(svg);

    const surfaces = new Map<
      HTMLElement,
      { filter: SVGFilterElement; id: string; size: string }
    >();
    const reduced = () =>
      reducedTransparency.matches || increasedContrast.matches;
    const update = (surface: HTMLElement, width: number, height: number, radius: number) => {
      const record = surfaces.get(surface);
      if (!record) return;
      surface.dataset.optics = reduced()
        ? "reduced"
        : canLens
        ? "lens"
        : "fallback";
      if (!canLens || reduced()) {
        surface.style.removeProperty("--glass-filter");
        return;
      }
      if (!width || !height) return;
      const size = `${width},${height},${radius}`;
      if (record.size !== size) {
        record.size = size;
        record.filter.replaceChildren();
        record.filter.setAttribute("width", String(width));
        record.filter.setAttribute("height", String(height));
        const image = document.createElementNS(svgNS, "feImage");
        image.setAttribute("href", makeLensMap(width, height, radius));
        image.setAttribute("width", String(width));
        image.setAttribute("height", String(height));
        image.setAttribute("preserveAspectRatio", "none");
        image.setAttribute("result", "lensMap");
        const scatter = document.createElementNS(svgNS, "feGaussianBlur");
        scatter.setAttribute("in", "SourceGraphic");
        scatter.setAttribute(
          "stdDeviation",
          surface.matches(".inspector,.scene-panel,.add-menu") ? "4" : "1.5",
        );
        scatter.setAttribute("result", "scattered");
        const lens = document.createElementNS(svgNS, "feDisplacementMap");
        lens.setAttribute("in", "scattered");
        lens.setAttribute("in2", "lensMap");
        lens.setAttribute("scale", "32");
        lens.setAttribute("xChannelSelector", "R");
        lens.setAttribute("yChannelSelector", "G");
        record.filter.append(image, scatter, lens);
      }
      surface.style.setProperty("--glass-filter", `url("#${record.id}")`);
    };
    // Read every size before writing filters: interleaved reads would force
    // expensive backdrop compositing for each successive surface.
    const updateAll = (elements: HTMLElement[]) => {
      const measurements = elements.map((surface) => ({
        surface,
        width: Math.round(surface.clientWidth),
        height: Math.round(surface.clientHeight),
        radius: parseFloat(getComputedStyle(surface).borderTopLeftRadius) || 24,
      }));
      measurements.forEach(({ surface, width, height, radius }) =>
        update(surface, width, height, radius),
      );
    };
    const resize = new ResizeObserver((entries) =>
      updateAll(entries.map(({ target }) => target as HTMLElement)),
    );
    const scan = () => {
      const added: HTMLElement[] = [];
      for (const [surface, { filter }] of surfaces)
        if (!surface.isConnected) {
          resize.unobserve(surface);
          filter.remove();
          surfaces.delete(surface);
        }
      container.querySelectorAll<HTMLElement>(".glass").forEach((surface) => {
        if (surfaces.has(surface)) return;
        const id = `spatial-lens-${nextId++}`;
        const filter = document.createElementNS(svgNS, "filter");
        filter.setAttribute("id", id);
        filter.setAttribute("filterUnits", "userSpaceOnUse");
        filter.setAttribute("x", "0");
        filter.setAttribute("y", "0");
        filter.setAttribute("color-interpolation-filters", "sRGB");
        defs.append(filter);
        surfaces.set(surface, { filter, id, size: "" });
        resize.observe(surface);
        added.push(surface);
      });
      if (added.length) updateAll(added);
    };
    scan();
    const observer = new MutationObserver((records) => {
      if (records.some((record) => !svg.contains(record.target))) scan();
    });
    // Observe UI insertion/removal, not the filter DOM we maintain ourselves.
    observer.observe(container, { childList: true, subtree: true });
    const preferencesChanged = () =>
      updateAll([...surfaces.keys()]);
    reducedTransparency.addEventListener("change", preferencesChanged);
    increasedContrast.addEventListener("change", preferencesChanged);
    const pointer = (event: PointerEvent) => {
      if (reducedMotion.matches || reduced()) return;
      const surface = (event.target as Element).closest<HTMLElement>(".glass");
      if (!surface) return;
      const rect = surface.getBoundingClientRect();
      surface.style.setProperty(
        "--light-x",
        `${((event.clientX - rect.left) / rect.width) * 100}%`,
      );
      surface.style.setProperty(
        "--light-y",
        `${((event.clientY - rect.top) / rect.height) * 100}%`,
      );
    };
    container.addEventListener("pointermove", pointer, { passive: true });
    return () => {
      observer.disconnect();
      resize.disconnect();
      svg.remove();
      surfaces.forEach((_, surface) => {
        surface.style.removeProperty("--glass-filter");
        delete surface.dataset.optics;
      });
      container.removeEventListener("pointermove", pointer);
      reducedTransparency.removeEventListener("change", preferencesChanged);
      increasedContrast.removeEventListener("change", preferencesChanged);
    };
  }, [root]);
}

// Parameter widgets generated from an operator spec's parameter type (units, kinds and ranges come
// from the engine, none are written here). Used by the inspector for nodes and for instance overrides.
import { fromDegrees, show, toDegrees } from "./graph.js";

/// A new element with a class and optional text.
export const el = (tag, className, text) => {
  const node = globalThis.document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
};

/// How a number is shown: lengths in metres, angles in degrees, one pixel of dragging moves `drag`.
const UNITS = {
  length: { suffix: "m", to: (v) => v, from: (v) => v, drag: 0.001 },
  angle: { suffix: "°", to: toDegrees, from: fromDegrees, drag: 1 },
  ratio: { suffix: "", to: (v) => v, from: (v) => v, drag: 0.01 },
  none: { suffix: "", to: (v) => v, from: (v) => v, drag: 0.01 },
};

/// A number field in display units. Dragging the caption scrubs it; one edit is one command.
function numberField(value, unit, whole, commit) {
  const u = UNITS[unit] ?? UNITS.none;
  const input = el("input", "num");
  input.type = "number";
  input.step = whole ? "1" : "any";
  input.value = show(u.to(value));
  input.addEventListener("change", () => {
    const typed = Number(input.value);
    if (input.value.trim() === "" || !Number.isFinite(typed)) { input.value = show(u.to(value)); return; }
    commit(whole ? Math.round(typed) : u.from(typed));
  });
  const wrap = el("span", "field");
  wrap.append(input);
  if (u.suffix) wrap.append(el("span", "unit", u.suffix));
  const scrub = (caption) => {
    caption.classList.add("scrub");
    caption.addEventListener("pointerdown", (down) => {
      const start = Number(input.value);
      const per = whole ? 0.1 : u.drag;
      caption.setPointerCapture(down.pointerId);
      const move = (event) => { input.value = show(start + (event.clientX - down.clientX) * per); };
      const up = () => {
        caption.removeEventListener("pointermove", move);
        caption.removeEventListener("pointerup", up);
        if (Number(input.value) !== start) input.dispatchEvent(new Event("change"));
      };
      caption.addEventListener("pointermove", move);
      caption.addEventListener("pointerup", up);
    });
  };
  return { wrap, input, scrub };
}

/// The control for a parameter (`param` is an operator spec's parameter) showing `value`; `commit(next)`
/// is called with a new value in the document's own units. The result has `control` and, for numbers,
/// `scrub(caption)` to make a caption drag the value.
export function widget(param, value, commit) {
  const ty = param.ty;
  const unit = ty.unit ?? "none";
  switch (ty.type) {
    case "num": case "int": {
      const field = numberField(value, unit, ty.type === "int", commit);
      return { control: field.wrap, scrub: field.scrub };
    }
    case "bool": {
      const box = el("input");
      box.type = "checkbox";
      box.checked = !!value;
      box.addEventListener("change", () => commit(box.checked));
      return { control: box };
    }
    case "choice": {
      const select = el("select");
      for (const option of ty.options) select.append(new Option(option, option, false, option === value));
      select.addEventListener("change", () => commit(select.value));
      return { control: select };
    }
    case "str": {
      const input = el("input");
      input.type = "text";
      input.value = value ?? "";
      input.addEventListener("change", () => commit(input.value));
      return { control: input };
    }
    case "vec2": case "vec3": {
      const group = el("span", "vec");
      const count = ty.type === "vec2" ? 2 : 3;
      const parts = Array.from({ length: count }, (_, i) => numberField(value[i], unit, false, (v) => {
        commit(parts.map((part, k) => (k === i ? v : (UNITS[unit] ?? UNITS.none).from(Number(part.input.value)))));
      }));
      group.append(...parts.map((part) => part.wrap));
      return { control: group };
    }
    default: { // points2: a list of [x, y]; edited as JSON
      const area = el("textarea", "points");
      area.value = JSON.stringify(value);
      area.rows = 3;
      area.spellcheck = false;
      area.addEventListener("change", () => {
        try { commit(JSON.parse(area.value)); }
        catch { area.classList.add("invalid"); }
      });
      return { control: area };
    }
  }
}

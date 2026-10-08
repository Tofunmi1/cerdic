import { Box, Circle, Layers, Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import type { SceneCommand, SceneObject, Vec3 } from "../../shared/commands";

function NumberField({
  value,
  onCommit,
  label,
  positive = false,
}: {
  value: number;
  onCommit: (value: number) => void;
  label: string;
  positive?: boolean;
}) {
  const [draft, setDraft] = useState(String(Number(value.toFixed(3))));
  useEffect(() => setDraft(String(Number(value.toFixed(3)))), [value]);
  const finish = () => {
    const next = Number(draft);
    if (draft.trim() && Number.isFinite(next) && (!positive || next >= 0.02))
      onCommit(next);
    else setDraft(String(Number(value.toFixed(3))));
  };
  return (
    <input
      aria-label={label}
      type="number"
      step="0.1"
      min={positive ? 0.02 : undefined}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={finish}
      onKeyDown={(e) => {
        if (e.key === "Enter") e.currentTarget.blur();
      }}
    />
  );
}
export function FactoryIcon({
  factory,
  size = 16,
}: {
  factory: SceneObject["factory"];
  size?: number;
}) {
  return factory === "box" ? (
    <Box size={size} />
  ) : factory === "cylinder" ? (
    <Circle size={size} />
  ) : (
    <Layers size={size} />
  );
}

export default function Inspector({
  object,
  onCommands,
}: {
  object: SceneObject | undefined;
  onCommands: (commands: SceneCommand[]) => void;
}) {
  const [name, setName] = useState(object?.name ?? "");
  const [hex, setHex] = useState(object?.color ?? "");
  useEffect(() => {
    setName(object?.name ?? "");
    setHex(object?.color ?? "");
  }, [object]);
  if (!object)
    return (
      <aside className="inspector glass">
        <div className="panel-title">
          <span>Properties</span>
          <span className="muted">No selection</span>
        </div>
        <div className="empty-inspector">
          <Box size={28} />
          <h3>A closer look.</h3>
          <p>Select an object in the viewport or the scene list to edit it.</p>
        </div>
      </aside>
    );
  const update = (patch: Partial<Omit<SceneObject, "id">>) =>
    onCommands([{ type: "update", id: object.id, patch }]);
  return (
    <aside className="inspector glass">
      <div className="panel-title">
        <span>Properties</span>
        <span className="tiny-pill">{object.factory}</span>
      </div>
      <div className="object-heading">
        <div className="object-preview" style={{ color: object.color }}>
          <FactoryIcon factory={object.factory} size={27} />
        </div>
        <div>
          <h3>{object.name}</h3>
          <code title={object.id}>{object.id}</code>
        </div>
      </div>
      <label className="field-label">
        Name
        <input
          aria-label="Object name"
          value={name}
          maxLength={100}
          onChange={(e) => setName(e.target.value)}
          onBlur={() => {
            if (name.trim()) update({ name: name.trim() });
            else setName(object.name);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
      </label>
      <div className="section-title">
        Transform <span>WORLD</span>
      </div>
      {(["position", "rotation", "scale"] as const).map((property) => (
        <fieldset key={property}>
          <legend>
            {property === "rotation"
              ? "Rotation · degrees"
              : property === "position"
              ? "Position · meters"
              : "Scale"}
          </legend>
          <div className="vector-inputs">
            {(["X", "Y", "Z"] as const).map((axis, i) => (
              <label key={axis}>
                <span className={`axis-${axis.toLowerCase()}`}>{axis}</span>
                <NumberField
                  label={`${property} ${axis}`}
                  positive={property === "scale"}
                  value={
                    property === "rotation"
                      ? (object[property][i] * 180) / Math.PI
                      : object[property][i]
                  }
                  onCommit={(value) => {
                    const vector = [...object[property]] as Vec3;
                    vector[i] =
                      property === "rotation" ? (value * Math.PI) / 180 : value;
                    update({ [property]: vector });
                  }}
                />
              </label>
            ))}
          </div>
        </fieldset>
      ))}
      <div className="section-title">
        Material <span>SOLID</span>
      </div>
      <div className="color-fields">
        <label className="color-picker" style={{ background: object.color }}>
          <input
            aria-label="Object colour"
            type="color"
            value={object.color}
            onChange={(e) => update({ color: e.target.value })}
          />
        </label>
        <input
          aria-label="Hex colour"
          value={hex}
          maxLength={7}
          onChange={(e) => setHex(e.target.value)}
          onBlur={() => {
            if (/^#[a-f\d]{6}$/i.test(hex)) update({ color: hex });
            else setHex(object.color);
          }}
          onKeyDown={(e) => {
            if (e.key === "Enter") e.currentTarget.blur();
          }}
        />
        <span className="muted">100%</span>
      </div>
      <div className="swatches">
        {["#a99bd2", "#e7a98d", "#8ca8df", "#8ebca3", "#ebcd80", "#eeeef1"].map(
          (color) => (
            <button
              key={color}
              aria-label={`Set colour ${color}`}
              style={{ background: color }}
              onClick={() => update({ color })}
            />
          ),
        )}
      </div>
      <button
        className="delete-object"
        onClick={() => onCommands([{ type: "delete", id: object.id }])}
      >
        <Trash2 size={14} /> Delete object
      </button>
    </aside>
  );
}

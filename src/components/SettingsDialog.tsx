import { useEffect, useId, useState } from "react";
import type { Preferences } from "../editor/preferences";
import { Modal } from "./Modal";

interface SettingsDialogProps {
  preferences: Preferences;
  onChange(nextPreferences: Preferences): void;
  onReset(): void;
  onClose(): void;
}

function NumericPreference({ label, value, min, max, step, hint, onChange }: {
  label: string; value: number; min: number; max: number; step: number; hint: string; onChange(value: number): void;
}) {
  const [draft, setDraft] = useState(String(value));
  const hintId = useId();
  useEffect(() => setDraft(String(value)), [value]);
  return <label className="settings-field">{label}<input aria-label={label} aria-describedby={hintId} type="number" min={min} max={max} step={step} value={draft}
    onChange={(event) => {
      setDraft(event.target.value);
      const next = event.target.valueAsNumber;
      if (Number.isFinite(next) && next >= min && next <= max) onChange(next);
    }} onBlur={() => setDraft(String(value))} /><span id={hintId} className="setting-hint">{hint}</span></label>;
}

export function SettingsDialog({ preferences, onChange, onReset, onClose }: SettingsDialogProps) {
  return <Modal title="Settings" description="Make Folio feel right for you. Changes are saved automatically." onClose={onClose} className="settings-dialog">
    <div className="settings-fields">
      <label className="settings-field">Theme<select value={preferences.theme} onChange={(event) => onChange({ ...preferences, theme: event.target.value as Preferences["theme"] })}>
        <option value="system">System</option><option value="light">Light</option><option value="dark">Dark</option>
      </select></label>
      <label className="settings-field">Page view<select value={preferences.viewMode} onChange={(event) => onChange({ ...preferences, viewMode: event.target.value as Preferences["viewMode"] })}>
        <option value="continuous">Continuous</option><option value="single">Single page</option>
      </select></label>
      <NumericPreference label="Default zoom" value={preferences.defaultZoom} min={50} max={200} step={5} hint="Percent, from 50 to 200" onChange={(defaultZoom) => onChange({ ...preferences, defaultZoom })} />
      <label className="settings-field">Pen color<input type="color" value={preferences.penColor} onChange={(event) => onChange({ ...preferences, penColor: event.target.value })} /></label>
      <NumericPreference label="Pen width" value={preferences.penWidth} min={0.5} max={20} step={0.5} hint="Points, from 0.5 to 20" onChange={(penWidth) => onChange({ ...preferences, penWidth })} />
    </div>
    <footer className="settings-actions"><button className="button subtle" onClick={onReset}>Reset defaults</button><button className="button primary" onClick={onClose}>Done</button></footer>
  </Modal>;
}

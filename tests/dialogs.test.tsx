import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { StrictMode, useLayoutEffect, useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ConfirmDialog } from "../src/components/ConfirmDialog";
import { SettingsDialog } from "../src/components/SettingsDialog";
import { SignaturePad } from "../src/components/SignaturePad";
import { usePreferences } from "../src/editor/preferences";

afterEach(() => { cleanup(); localStorage.clear(); });

function ConfirmationExample() {
  const [open, setOpen] = useState(false);
  const [result, setResult] = useState("Unchanged");
  return <><button onClick={() => setOpen(true)}>Open decision</button><output>{result}</output>{open && <ConfirmDialog title="Discard edits?" description="Unsaved work will be lost." confirmLabel="Discard" onCancel={() => setOpen(false)} onConfirm={() => { setResult("Discarded"); setOpen(false); }} />}</>;
}

describe("shared dialogs", () => {
  it("focuses cancel, traps both tab directions, and restores the trigger after Escape", async () => {
    render(<ConfirmationExample />);
    const trigger = screen.getByRole("button", { name: "Open decision" });
    trigger.focus();
    fireEvent.click(trigger);
    const cancel = screen.getByRole("button", { name: "Keep editing" });
    const confirm = screen.getByRole("button", { name: "Discard" });
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(cancel, { key: "Tab", shiftKey: true });
    expect(confirm).toHaveFocus();
    fireEvent.keyDown(confirm, { key: "Tab" });
    expect(cancel).toHaveFocus();
    fireEvent.keyDown(cancel, { key: "Escape" });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(screen.getByText("Unchanged")).toBeInTheDocument();
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("preserves the original opener across Strict Mode effect replay", async () => {
    render(<StrictMode><button>Other action</button><ConfirmationExample /></StrictMode>);
    const trigger = screen.getByText("Open decision");
    trigger.focus();
    fireEvent.click(trigger);
    await waitFor(() => expect(screen.getByText("Keep editing")).toHaveFocus());
    fireEvent.keyDown(screen.getByText("Keep editing"), { key: "Escape" });
    await waitFor(() => expect(trigger).toHaveFocus());
  });
  it("makes background content inert and restores its original state on close", () => {
    const alreadyInert = document.createElement("aside");
    alreadyInert.setAttribute("inert", "existing");
    document.body.append(alreadyInert);
    try {
      const { container } = render(<ConfirmationExample />);
      fireEvent.click(screen.getByText("Open decision"));
      expect(container).toHaveAttribute("inert");
      expect(screen.getByRole("dialog").closest(".modal-backdrop")).not.toHaveAttribute("inert");
      fireEvent.click(screen.getByText("Keep editing"));
      expect(container).not.toHaveAttribute("inert");
      expect(alreadyInert).toHaveAttribute("inert", "existing");
    } finally { alreadyInert.remove(); }
  });

  it("restores focus after the opener becomes enabled during close commit", async () => {
    function Example() {
      const [open, setOpen] = useState(false);
      const [disabled, setDisabled] = useState(false);
      useLayoutEffect(() => setDisabled(open), [open]);
      return <>{open && <ConfirmDialog title="Discard edits?" description="Unsaved work will be lost." confirmLabel="Discard" onCancel={() => setOpen(false)} onConfirm={() => setOpen(false)} />}<button disabled={disabled} onClick={() => setOpen(true)}>Open decision</button></>;
    }
    render(<Example />);
    const trigger = screen.getByRole("button", { name: "Open decision" });
    trigger.focus();
    fireEvent.click(trigger);
    expect(trigger).toBeDisabled();
    fireEvent.keyDown(screen.getByText("Keep editing"), { key: "Escape" });
    expect(trigger).toBeEnabled();
    await waitFor(() => expect(trigger).toHaveFocus());
  });

  it("focuses an available control when the original opener was removed", async () => {
    function Example() {
      const [open, setOpen] = useState(false);
      return <>{!open && <button onClick={() => setOpen(true)}>Open decision</button>}<button>Available action</button>{open && <ConfirmDialog title="Discard edits?" description="Unsaved work will be lost." confirmLabel="Discard" onCancel={() => setOpen(false)} onConfirm={() => setOpen(false)} />}</>;
    }
    render(<Example />);
    const originalTrigger = screen.getByText("Open decision");
    originalTrigger.focus();
    fireEvent.click(originalTrigger);
    fireEvent.keyDown(screen.getByText("Keep editing"), { key: "Escape" });
    await waitFor(() => expect(screen.getByText("Open decision")).toHaveFocus());
  });
  it("keeps editor keyboard shortcuts from bubbling outside a dialog", () => {
    const shortcut = vi.fn();
    window.addEventListener("keydown", shortcut);
    try {
      render(<ConfirmationExample />);
      fireEvent.click(screen.getByText("Open decision"));
      fireEvent.keyDown(screen.getByText("Keep editing"), { key: "z", ctrlKey: true });
      expect(shortcut).not.toHaveBeenCalled();
      fireEvent.click(screen.getByRole("button", { name: "Discard" }));
      expect(screen.getByText("Discarded")).toBeInTheDocument();
    } finally { window.removeEventListener("keydown", shortcut); }
  });

  it("keeps focus inside when another element attempts to take focus", () => {
    render(<ConfirmationExample />);
    const trigger = screen.getByText("Open decision");
    fireEvent.click(trigger);
    trigger.focus();
    expect(screen.getByText("Keep editing")).toHaveFocus();
  });

  it("uses the shared Escape cancellation for signatures", () => {
    function Example() {
      const [open, setOpen] = useState(true);
      return open ? <SignaturePad onCancel={() => setOpen(false)} onAccept={() => {}} /> : <p>Cancelled</p>;
    }
    render(<Example />);
    fireEvent.keyDown(screen.getByRole("button", { name: "Cancel" }), { key: "Escape" });
    expect(screen.getByText("Cancelled")).toBeInTheDocument();
  });

  it("allows intermediate numeric input and restores invalid values on blur", () => {
    function Example() {
      const { preferences, setPreferences, resetPreferences } = usePreferences();
      return <SettingsDialog preferences={preferences} onChange={setPreferences} onReset={resetPreferences} onClose={() => {}} />;
    }
    render(<Example />);
    const zoom = screen.getByLabelText("Default zoom");
    fireEvent.change(zoom, { target: { value: "" } });
    expect(zoom).toHaveValue(null);
    fireEvent.change(zoom, { target: { value: "1" } });
    expect(zoom).toHaveValue(1);
    fireEvent.change(zoom, { target: { value: "125" } });
    fireEvent.blur(zoom);
    expect(zoom).toHaveValue(125);
    fireEvent.change(zoom, { target: { value: "999" } });
    fireEvent.blur(zoom);
    expect(zoom).toHaveValue(125);
  });
  it("edits settings immediately and resets every preference", () => {
    function Example() {
      const { preferences, setPreferences, resetPreferences } = usePreferences();
      return <SettingsDialog preferences={preferences} onChange={setPreferences} onReset={resetPreferences} onClose={() => {}} />;
    }
    render(<Example />);
    fireEvent.change(screen.getByLabelText("Theme"), { target: { value: "dark" } });
    expect(document.documentElement.dataset.theme).toBe("dark");
    fireEvent.change(screen.getByLabelText("Page view"), { target: { value: "single" } });
    fireEvent.change(screen.getByLabelText("Default zoom"), { target: { value: "125" } });
    fireEvent.change(screen.getByLabelText("Pen color"), { target: { value: "#123456" } });
    fireEvent.change(screen.getByLabelText("Pen width"), { target: { value: "4" } });
    expect(screen.getByLabelText("Page view")).toHaveValue("single");
    expect(screen.getByLabelText("Default zoom")).toHaveValue(125);
    expect(screen.getByLabelText("Pen color")).toHaveValue("#123456");
    expect(screen.getByLabelText("Pen width")).toHaveValue(4);
    fireEvent.click(screen.getByRole("button", { name: "Reset defaults" }));
    expect(screen.getByLabelText("Theme")).toHaveValue("system");
    expect(screen.getByLabelText("Page view")).toHaveValue("continuous");
    expect(screen.getByLabelText("Default zoom")).toHaveValue(90);
    expect(screen.getByLabelText("Pen width")).toHaveValue(2);
  });
});

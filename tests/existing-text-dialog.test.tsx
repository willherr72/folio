import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ExistingTextDialog } from "../src/components/ExistingTextDialog";
import type { EditableTextRun } from "../src/editor/types";

const run: EditableTextRun = { objectIndex: 2, text: "Original text", fontName: "Helvetica", fontSize: 12, bounds: { x: 20, y: 30, width: 100, height: 12 }, supported: true };
const options = () => ({ run, busy: false, error: null, onApply: vi.fn(), onCancel: vi.fn() });
afterEach(cleanup);

describe("existing text dialog", () => {
  it("focuses and selects the original, then submits the replacement through its form", () => {
    const props = options();
    render(<ExistingTextDialog {...props} />);
    const input = screen.getByRole("textbox", { name: "Replacement text" }) as HTMLInputElement;
    expect(input).toHaveFocus();
    expect(input.selectionStart).toBe(0);
    expect(input.selectionEnd).toBe(run.text.length);
    expect(input).toHaveAttribute("maxlength", "1000");
    expect(screen.getByText(/Helvetica.*12/)).toBeInTheDocument();
    expect(screen.getByText(/extend into available space/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply changes" })).toBeDisabled();
    fireEvent.change(input, { target: { value: "Updated text" } });
    fireEvent.submit(input.closest("form")!);
    expect(props.onApply).toHaveBeenCalledWith("Updated text");
  });

  it.each(["", "   ", "Original text", "control\u0001text", "x".repeat(1001)])("rejects invalid or unchanged replacement %j", value => {
    const props = options();
    render(<ExistingTextDialog {...props} />);
    const input = screen.getByRole("textbox", { name: "Replacement text" });
    fireEvent.change(input, { target: { value } });
    expect(screen.getByRole("button", { name: "Apply changes" })).toBeDisabled();
    fireEvent.submit(input.closest("form")!);
    expect(props.onApply).not.toHaveBeenCalled();
  });

  it("preserves typed text after an error and blocks escape, backdrop, cancellation and resubmission while busy", () => {
    const props = options();
    const view = render(<ExistingTextDialog {...props} />);
    const input = screen.getByRole("textbox", { name: "Replacement text" });
    fireEvent.change(input, { target: { value: "Longer replacement" } });
    view.rerender(<ExistingTextDialog {...props} busy />);
    fireEvent.keyDown(input, { key: "Escape" });
    fireEvent.pointerDown(screen.getByRole("dialog").parentElement!);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    fireEvent.submit(input.closest("form")!);
    expect(props.onCancel).not.toHaveBeenCalled();
    expect(props.onApply).not.toHaveBeenCalled();
    view.rerender(<ExistingTextDialog {...props} error="Replacement does not fit the existing text run." />);
    expect(input).toHaveValue("Longer replacement");
    expect(screen.getByRole("alert")).toHaveTextContent("does not fit");
    expect(screen.getByRole("button", { name: "Apply changes" })).toBeEnabled();
    fireEvent.keyDown(input, { key: "Escape" });
    expect(props.onCancel).toHaveBeenCalledOnce();
  });

  it("shows unsupported text and its reason without offering a replacement action", () => {
    const props = options();
    render(<ExistingTextDialog {...props} run={{ ...run, supported: false, reason: "Subset font cannot be preserved." }} />);
    expect(screen.getByText("Subset font cannot be preserved.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Apply changes" })).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    expect(props.onApply).not.toHaveBeenCalled();
  });

  it("uses the shared modal focus trap and restores the opener when dismissed", async () => {
    const opener = document.createElement("button");
    document.body.append(opener);
    opener.focus();
    try {
      const view = render(<ExistingTextDialog {...options()} />);
      const input = screen.getByRole("textbox", { name: "Replacement text" });
      fireEvent.change(input, { target: { value: "New text" } });
      fireEvent.keyDown(input, { key: "Tab", shiftKey: true });
      expect(screen.getByRole("button", { name: "Apply changes" })).toHaveFocus();
      fireEvent.keyDown(document.activeElement!, { key: "Tab" });
      expect(input).toHaveFocus();
      view.unmount();
      await waitFor(() => expect(opener).toHaveFocus());
    } finally { opener.remove(); }
  });
});

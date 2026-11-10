import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ExistingTextDialog } from "../src/components/ExistingTextDialog";
import type { EditableTextRun } from "../src/editor/types";

const run: EditableTextRun = { objectIndex: 2, text: "Original text", fontName: "Helvetica", fontSize: 12, bounds: { x: 20, y: 30, width: 100, height: 12 }, supported: true };
const options = () => ({ run, busy: false, error: null, onApply: vi.fn(), onCancel: vi.fn() });
afterEach(cleanup);

describe("existing text dialog", () => {
  it("passes the exact replacement and caret to paragraph preparation",()=>{
    const props=options(),onEditParagraph=vi.fn();
    render(<ExistingTextDialog {...props} onEditParagraph={onEditParagraph}/>);
    const input=screen.getByRole("textbox",{name:"Replacement text"}) as HTMLInputElement;
    fireEvent.change(input,{target:{value:"Original this text"}});
    input.setSelectionRange(9,13,"backward");
    fireEvent.click(screen.getByRole("button",{name:"Edit paragraph…"}));
    expect(onEditParagraph).toHaveBeenCalledWith({originalText:run.text,replacement:"Original this text",selection:{start:9,end:13,direction:"backward"},font:null});
    expect(props.onApply).not.toHaveBeenCalled();
  });
  it("shows the measured physical size without changing replacement identity",()=>{
    const props=options();const measured={...run,fontSize:1,displayFontSize:12};
    render(<ExistingTextDialog {...props} run={measured}/>);
    expect(screen.getByText("Helvetica · 12 pt")).toBeInTheDocument();
    fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"Changed"}});
    fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));expect(props.onApply).toHaveBeenCalledWith("Changed");expect(measured.fontSize).toBe(1);
  });
  it.each(["Old", " Old ", "Old  "])("uses exact source text %j while retaining native selection identity",sourceText=>{
    const props=options();const identity={...run,text:"Old ",sourceText};
    render(<ExistingTextDialog {...props} run={identity}/>);
    const input=screen.getByRole("textbox",{name:"Replacement text"});
    expect(input).toHaveValue(sourceText);expect(screen.getByRole("button",{name:"Apply changes"})).toBeDisabled();
    fireEvent.change(input,{target:{value:"New "}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
    expect(props.onApply).toHaveBeenCalledWith("New ");expect(identity.text).toBe("Old ");
  });
  it("explains spacing limits while allowing an original-font edit",()=>{
    const props=options();
    const reason="Keep the original font to preserve this text’s word spacing.";
    const spaced={...run,objectPath:[1,0,0],canSubstitute:false,fontSubstitutionReason:reason};
    render(<ExistingTextDialog {...props} run={spaced}/>);
    expect(screen.getByText(reason)).toBeInTheDocument();
    expect(screen.queryByRole("button",{name:/Choose substitute/})).not.toBeInTheDocument();
    const input=screen.getByRole("textbox",{name:"Replacement text"});
    expect(input).not.toHaveAttribute("readonly");
    fireEvent.change(input,{target:{value:"Original text added"}});
    fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));
    expect(props.onApply).toHaveBeenCalledWith("Original text added");
  });
  it("edits accented embedded form text with readable font and occurrence-specific limits",()=>{
    const props=options();const embedded={...run,objectPath:[1,0,0],text:"Café old",fontName:"ABCDEF+DejaVuSerif",fontDisplayName:"DejaVu Serif",isEmbedded:true,canSubstitute:false};
    render(<ExistingTextDialog {...props} run={embedded} onEditTogether={vi.fn()} onEditParagraph={vi.fn()}/>);
    expect(screen.getByText(/DejaVu Serif.*12/)).toBeInTheDocument();expect(screen.getByText(/Font substitution is not available for this occurrence/)).toBeInTheDocument();
    expect(screen.queryByRole("button",{name:/Choose substitute|Edit together/})).not.toBeInTheDocument();
    expect(screen.getByRole("button",{name:/Edit paragraph/})).toBeEnabled();
    fireEvent.change(screen.getByRole("textbox",{name:"Replacement text"}),{target:{value:"Café edit"}});fireEvent.click(screen.getByRole("button",{name:"Apply changes"}));expect(props.onApply).toHaveBeenCalledWith("Café edit");
  });
  it("prefers a verified display label to the PDF resource name", () => {
    const labelled={...run,fontName:`Folio${"b".repeat(64)}`,fontDisplayName:"DejaVu Serif"};
    render(<ExistingTextDialog {...options()} run={labelled}/>);
    expect(screen.getByText(/DejaVu Serif.*12/)).toBeInTheDocument();
    expect(screen.queryByText(new RegExp(labelled.fontName))).not.toBeInTheDocument();
  });
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

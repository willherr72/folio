import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { App } from "../src/App";

afterEach(cleanup);

describe("Folio workspace", () => {
  it("opens sample content only after the explicit demo action", async () => {
    render(<App initialDemo={false} />);

    expect(screen.getByRole("heading", { name: "Make PDFs feel finished." })).toBeInTheDocument();
    expect(screen.queryByText("Folio welcome.pdf")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Explore demo" }));

    await waitFor(() => expect(screen.getByText("Folio welcome.pdf")).toBeInTheDocument());
    expect(screen.getByLabelText("Page 1 of 3")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Export demo plan" })).toBeInTheDocument();
  });

  it("uses Ctrl+Shift+Z as redo without also undoing", async () => {
    render(<App initialDemo={false} />);
    fireEvent.click(screen.getByRole("button", { name: "Explore demo" }));
    await waitFor(() => expect(screen.getByText("Folio welcome.pdf")).toBeInTheDocument());

    const rotate = screen.getByRole("button", { name: "Rotate" });
    fireEvent.click(rotate);
    fireEvent.click(rotate);
    expect(screen.getByText("180° clockwise")).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "z", ctrlKey: true });
    expect(screen.getByText("90° clockwise")).toBeInTheDocument();
    fireEvent.keyDown(window, { key: "z", ctrlKey: true, shiftKey: true });

    expect(screen.getByText("180° clockwise")).toBeInTheDocument();
  });
});
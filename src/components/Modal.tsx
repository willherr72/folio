import { useId, useLayoutEffect, useRef, type ReactNode, type RefObject, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";

interface ModalProps {
  title: string;
  children: ReactNode;
  onClose(): void;
  className?: string;
  description?: string;
  initialFocusRef?: RefObject<HTMLElement>;
}

const focusableSelector = 'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

export function Modal({ title, children, onClose, className = "", description, initialFocusRef }: ModalProps) {
  const dialogRef = useRef<HTMLElement>(null);
  const initialFocus = useRef(initialFocusRef);
  const openerRef = useRef<HTMLElement | null>(null);
  const headingId = useId();
  const descriptionId = useId();

  useLayoutEffect(() => {
    if (!openerRef.current && document.activeElement instanceof HTMLElement) openerRef.current = document.activeElement;
    const previousFocus = openerRef.current;
    const dialog = dialogRef.current;
    const focusInside = () => (initialFocus.current?.current ?? dialog?.querySelector<HTMLElement>(focusableSelector) ?? dialog)?.focus();
    focusInside();
    const background = Array.from(document.body.children)
      .filter((element): element is HTMLElement => element instanceof HTMLElement && element !== dialog?.parentElement)
      .map((element) => ({ element, inert: element.getAttribute("inert") }));
    background.forEach(({ element }) => element.setAttribute("inert", ""));
    const retainFocus = (event: FocusEvent) => {
      if (event.target instanceof Node && !dialog?.contains(event.target)) focusInside();
    };
    document.addEventListener("focusin", retainFocus);
    return () => {
      document.removeEventListener("focusin", retainFocus);
      background.forEach(({ element, inert }) => {
        if (inert === null) element.removeAttribute("inert");
        else element.setAttribute("inert", inert);
      });
      // React removes the dialog and re-enables its opener during this commit.
      queueMicrotask(() => {
        if (dialog?.isConnected) return;
        const active = document.activeElement;
        if (active instanceof HTMLElement && active !== document.body && active.isConnected) return;
        const canFocus = (element: HTMLElement) => element.isConnected && !element.closest("[inert], [hidden]") && !element.matches(":disabled");
        const fallback = background.flatMap(({ element }) => Array.from(element.querySelectorAll<HTMLElement>(focusableSelector)));
        for (const target of [previousFocus, ...fallback]) {
          if (!target || target === document.body || !canFocus(target)) continue;
          target.focus({ preventScroll: true });
          if (document.activeElement === target) break;
        }
      });
    };
  }, []);

  const handleKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    event.stopPropagation();
    if (event.key === "Escape") { event.preventDefault(); onClose(); return; }
    if (event.key !== "Tab") return;
    const elements = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(focusableSelector) ?? [])
      .filter((element) => !element.hidden && element.getAttribute("aria-hidden") !== "true");
    const first = elements[0];
    const last = elements[elements.length - 1];
    if (!first) { event.preventDefault(); dialogRef.current?.focus(); }
    else if (event.shiftKey && (document.activeElement === first || document.activeElement === dialogRef.current)) {
      event.preventDefault(); last.focus();
    } else if (!event.shiftKey && (document.activeElement === last || document.activeElement === dialogRef.current)) {
      event.preventDefault(); first.focus();
    }
  };

  return createPortal(
    <div className="modal-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <section ref={dialogRef} className={`modal-dialog ${className}`} role="dialog" aria-modal="true" aria-labelledby={headingId}
        aria-describedby={description ? descriptionId : undefined} tabIndex={-1} onKeyDown={handleKeyDown}>
        <header className="modal-heading"><h2 id={headingId}>{title}</h2>{description && <p id={descriptionId}>{description}</p>}</header>
        {children}
      </section>
    </div>, document.body,
  );
}

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
  const headingId = useId();
  const descriptionId = useId();

  useLayoutEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const dialog = dialogRef.current;
    const focusInside = () => (initialFocus.current?.current ?? dialog?.querySelector<HTMLElement>(focusableSelector) ?? dialog)?.focus();
    focusInside();
    const retainFocus = (event: FocusEvent) => {
      if (event.target instanceof Node && !dialog?.contains(event.target)) focusInside();
    };
    document.addEventListener("focusin", retainFocus);
    return () => {
      document.removeEventListener("focusin", retainFocus);
      if (previousFocus?.isConnected) previousFocus.focus();
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

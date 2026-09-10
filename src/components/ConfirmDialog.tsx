import { useRef } from "react";
import { Modal } from "./Modal";

interface ConfirmDialogProps {
  title: string;
  description: string;
  confirmLabel: string;
  onConfirm(): void;
  onCancel(): void;
}

export function ConfirmDialog({ title, description, confirmLabel, onConfirm, onCancel }: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  return <Modal title={title} description={description} onClose={onCancel} className="confirm-dialog" initialFocusRef={cancelRef}>
    <footer className="dialog-actions"><button ref={cancelRef} className="button" onClick={onCancel}>Keep editing</button>
      <button className="button primary" onClick={onConfirm}>{confirmLabel}</button></footer>
  </Modal>;
}

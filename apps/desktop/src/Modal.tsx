import { useEffect, useRef, type ReactNode } from 'react';
export function Modal({
  children,
  label,
  onClose,
  className = '',
}: {
  children: ReactNode;
  label: string;
  onClose: () => void;
  className?: string;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  const latest = useRef(onClose);
  useEffect(() => {
    latest.current = onClose;
  }, [onClose]);
  useEffect(() => {
    const element = dialog.current;
    const previous = document.activeElement;
    element?.showModal();
    return () => {
      element?.close();
      if (previous instanceof HTMLElement) previous.focus();
    };
  }, []);
  return (
    <dialog
      className={`editor-modal ${className}`}
      ref={dialog}
      aria-label={label}
      onCancel={(e) => {
        e.preventDefault();
        latest.current();
      }}
    >
      {children}
    </dialog>
  );
}

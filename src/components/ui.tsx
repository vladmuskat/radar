import { createContext, useContext, useEffect, useRef, type ReactNode } from 'react';
import { X } from 'lucide-react';
export const ErrorContext = createContext('');

/** Renders a native modal with consistent title, close and error handling. */
export function Dialog({
  title,
  caption,
  onClose,
  children,
  wide = false,
}: {
  title: string;
  caption?: string;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const error = useContext(ErrorContext);
  useEffect(() => {
    ref.current?.showModal();
  }, []);
  return (
    <dialog
      ref={ref}
      className={`dialog ${wide ? 'wide' : ''}`}
      onCancel={(e) => {
        e.preventDefault();
        onClose();
      }}
    >
      <header>
        <div>
          <h2>{title}</h2>
          {caption && <p>{caption}</p>}
        </div>
        <button className="icon-button" aria-label="Закрыть" onClick={onClose}>
          <X size={20} />
        </button>
      </header>
      {error && (
        <p className="inline-error" role="alert">
          {error}
        </p>
      )}
      {children}
    </dialog>
  );
}
/** Renders one labelled result or statistics value. */
export function Metric({
  label,
  value,
  className = '',
}: {
  label: string;
  value: ReactNode;
  className?: string;
}) {
  return (
    <div className={`metric ${className}`}>
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

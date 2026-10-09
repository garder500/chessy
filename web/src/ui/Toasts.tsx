import { useT } from "../i18n";
import { store, type Toast } from "../store";

export function Toasts({ toasts }: { toasts: Toast[] }) {
  const t = useT();
  return (
    <div className="toasts" role="status" aria-live="polite">
      {toasts.map((toast) => (
        <div className="toast" key={toast.id}>
          <span>{toast.text}</span>
          <button type="button" className="toast-x" aria-label={t("sheet.close_toast")} onClick={() => store.dismissToast(toast.id)}>
            <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true" focusable="false">
              <path d="M2 2l8 8M10 2l-8 8" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
            </svg>
          </button>
        </div>
      ))}
    </div>
  );
}

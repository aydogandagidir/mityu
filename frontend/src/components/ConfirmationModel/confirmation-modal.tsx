import React from 'react';

interface ConfirmationModalProps {
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
  text: string;
  isOpen: boolean;
  isBusy?: boolean;
}

export function ConfirmationModal({ onConfirm, onCancel, text, isOpen, isBusy = false }: ConfirmationModalProps) {
  if (!isOpen) return null;

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div
        className="bg-card rounded-lg p-6 max-w-md w-full mx-4"
        role="dialog"
        aria-modal="true"
        aria-labelledby="delete-confirmation-title"
        aria-busy={isBusy}
      >
        <h2 id="delete-confirmation-title" className="text-xl font-semibold mb-4">Confirm Delete</h2>
        <p className="text-muted-foreground mb-6">{text}</p>
        <div className="flex justify-end space-x-4">
          <button
            type="button"
            onClick={onCancel}
            disabled={isBusy}
            className="px-4 py-2 text-muted-foreground hover:bg-muted rounded-md transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={() => void onConfirm()}
            disabled={isBusy}
            className="px-4 py-2 bg-red-600 text-white hover:bg-red-700 rounded-md transition-colors disabled:cursor-not-allowed disabled:opacity-50"
          >
            {isBusy ? 'Deleting…' : 'Delete'}
          </button>
        </div>
      </div>
    </div>
  );
}

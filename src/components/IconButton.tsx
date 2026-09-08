import type { ButtonHTMLAttributes, ReactNode } from 'react';

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  children: ReactNode;
  quiet?: boolean;
}

export function IconButton({ label, children, quiet, className = '', ...props }: IconButtonProps) {
  return (
    <button
      type="button"
      className={`icon-button ${quiet ? 'icon-button--quiet' : ''} ${className}`}
      aria-label={label}
      title={label}
      {...props}
    >
      {children}
    </button>
  );
}

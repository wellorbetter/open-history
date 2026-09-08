export function BrandMark({ size = 34 }: { size?: number }) {
  return (
    <span className="brand-mark" style={{ width: size, height: size }} aria-hidden="true">
      <svg viewBox="0 0 32 32" role="img">
        <path d="M16 5.3c.8 5.8 3 8 8.8 8.8-5.8.8-8 3-8.8 8.8-.8-5.8-3-8-8.8-8.8 5.8-.8 8-3 8.8-8.8Z" />
        <path d="M24.4 21.2c.3 2.2 1.2 3.1 3.4 3.4-2.2.3-3.1 1.2-3.4 3.4-.3-2.2-1.2-3.1-3.4-3.4 2.2-.3 3.1-1.2 3.4-3.4Z" />
      </svg>
    </span>
  );
}

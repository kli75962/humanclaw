import type { ReactNode } from 'react';

export function SegmentControl<T extends string>({
  options,
  value,
  onChange,
}: {
  options: { value: T; label: string }[];
  value: T;
  onChange: (v: T) => void;
}) {
  return (
    <div className="settings-segment">
      {options.map((opt) => (
        <button
          key={opt.value}
          onClick={() => onChange(opt.value)}
          className={`settings-segment-btn${value === opt.value ? ' settings-segment-btn-active' : ''}`}
        >
          {opt.label}
        </button>
      ))}
    </div>
  );
}

export function SectionHeader({ children }: { children: ReactNode }) {
  return <p className="settings-section-header">{children}</p>;
}

export function SectionFooter({ children }: { children: ReactNode }) {
  return <p className="settings-section-footer">{children}</p>;
}

export function Card({ children }: { children: ReactNode }) {
  return <div className="settings-card">{children}</div>;
}

export function CardRow({
  onClick,
  children,
}: {
  onClick?: () => void;
  children: ReactNode;
}) {
  const Tag = onClick ? 'button' : 'div';
  return (
    <Tag onClick={onClick} className="settings-card-row">
      {children}
    </Tag>
  );
}

export function CardDivider() {
  return <div className="settings-card-divider" />;
}

import clsx from "clsx";
import type { ReactNode } from "react";

/**
 * A scrolling page set like a stenographer's pad: a faint red margin rule, with labels
 * (dates, speakers, owners) hanging in the gutter to its left. Narrow windows drop the
 * gutter and stack each label above its line.
 */
export function PadPage({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className="@container h-full overflow-y-auto">
      <div className={clsx("relative mx-auto min-h-full max-w-[860px] px-8 pt-10 pb-24", className)}>
        <div
          aria-hidden
          className="pointer-events-none absolute inset-y-0 left-[calc(2rem+96px+28px)] hidden w-px bg-[var(--margin-soft)] @min-[620px]:block"
        />
        {children}
      </div>
    </div>
  );
}

interface PadRowProps {
  label?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Aligns the label with the first line of larger text. */
  labelClassName?: string;
}

export function PadRow({ label, children, className, labelClassName }: PadRowProps) {
  return (
    <div className={clsx("grid grid-cols-1 gap-1 @min-[620px]:grid-cols-[96px_minmax(0,620px)] @min-[620px]:gap-x-14", className)}>
      <div
        className={clsx(
          "text-[11.5px] leading-[1.35] text-graphite tabular @min-[620px]:pt-[3px] @min-[620px]:text-right",
          !label && "hidden @min-[620px]:block",
          labelClassName,
        )}
      >
        {label}
      </div>
      <div className="min-w-0">{children}</div>
    </div>
  );
}

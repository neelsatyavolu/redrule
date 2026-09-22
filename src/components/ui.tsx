import clsx from "clsx";
import { X } from "lucide-react";
import { Dialog as RadixDialog, Switch as RadixSwitch } from "radix-ui";
import { forwardRef, type ButtonHTMLAttributes, type InputHTMLAttributes, type ReactNode, type TextareaHTMLAttributes } from "react";

type Variant = "primary" | "record" | "quiet" | "ghost" | "danger";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-ink text-paper hover:bg-ink/88 active:bg-ink/80",
  record: "bg-margin text-white hover:bg-margin/90 active:bg-margin/80",
  quiet: "bg-wash text-ink hover:bg-rule active:bg-rule/80",
  ghost: "text-ink hover:bg-wash active:bg-rule",
  danger: "text-margin hover:bg-margin/10 active:bg-margin/15",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: "sm" | "md";
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = "quiet", size = "md", className, type = "button", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      className={clsx(
        "inline-flex items-center justify-center gap-1.5 rounded-[7px] font-medium whitespace-nowrap transition-colors duration-100 disabled:pointer-events-none disabled:opacity-40",
        size === "md" ? "h-8 px-3.5 text-[13px]" : "h-7 px-2.5 text-[12.5px]",
        VARIANTS[variant],
        className,
      )}
      {...props}
    />
  );
});

interface IconButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  label: string;
  active?: boolean;
}

/** A toolbar button. `label` is both its accessible name and its tooltip. */
export const IconButton = forwardRef<HTMLButtonElement, IconButtonProps>(function IconButton(
  { label, active, className, children, type = "button", ...props },
  ref,
) {
  return (
    <button
      ref={ref}
      type={type}
      aria-label={label}
      title={label}
      className={clsx(
        "grid size-8 place-items-center rounded-[7px] text-graphite transition-colors duration-100 hover:bg-wash hover:text-ink disabled:pointer-events-none disabled:opacity-35",
        active && "bg-wash text-ink",
        className,
      )}
      {...props}
    >
      {children}
    </button>
  );
});

export function Spinner({ className }: { className?: string }) {
  return (
    <svg className={clsx("size-3.5 animate-spin text-graphite", className)} viewBox="0 0 16 16" aria-hidden>
      <circle cx="8" cy="8" r="6.5" fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="2" />
      <path d="M14.5 8A6.5 6.5 0 0 0 8 1.5" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

export function RecordingDot({ size = 8 }: { size?: number }) {
  return (
    <span
      role="img"
      aria-label="Recording"
      className="recording-dot inline-block shrink-0 rounded-full bg-margin"
      style={{ width: size, height: size }}
    />
  );
}

interface SegmentedProps<T extends string> {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
  className?: string;
}

export function Segmented<T extends string>({ value, options, onChange, label, className }: SegmentedProps<T>) {
  return (
    <div role="radiogroup" aria-label={label} className={clsx("flex rounded-[8px] bg-wash p-0.5", className)}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          role="radio"
          aria-checked={option.value === value}
          onClick={() => onChange(option.value)}
          className={clsx(
            "h-6 flex-1 rounded-[6px] px-3 text-[12.5px] font-medium transition-colors duration-100",
            option.value === value ? "bg-raised text-ink shadow-[0_1px_2px_rgb(0_0_0/0.08)]" : "text-graphite hover:text-ink",
          )}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export const TextInput = forwardRef<HTMLInputElement, InputHTMLAttributes<HTMLInputElement>>(function TextInput(
  { className, ...props },
  ref,
) {
  return (
    <input
      ref={ref}
      className={clsx(
        "h-8 w-full rounded-[7px] border border-rule bg-raised px-2.5 text-[13px] text-ink placeholder:text-faint focus:border-focus focus:outline-none",
        className,
      )}
      {...props}
    />
  );
});

export const TextArea = forwardRef<HTMLTextAreaElement, TextareaHTMLAttributes<HTMLTextAreaElement>>(function TextArea(
  { className, ...props },
  ref,
) {
  return (
    <textarea
      ref={ref}
      className={clsx(
        "field-sizing-content min-h-16 w-full resize-none rounded-[7px] border border-rule bg-raised px-2.5 py-1.5 text-[13px] leading-relaxed text-ink placeholder:text-faint focus:border-focus focus:outline-none",
        className,
      )}
      {...props}
    />
  );
});

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (value: boolean) => void; label: string }) {
  return (
    <RadixSwitch.Root
      checked={checked}
      onCheckedChange={onChange}
      aria-label={label}
      className="relative h-[22px] w-[38px] shrink-0 rounded-full bg-rule transition-colors duration-150 data-[state=checked]:bg-focus"
    >
      <RadixSwitch.Thumb className="block size-[18px] translate-x-[2px] rounded-full bg-white shadow-[0_1px_2px_rgb(0_0_0/0.25)] transition-transform duration-150 data-[state=checked]:translate-x-[18px]" />
    </RadixSwitch.Root>
  );
}

interface DialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description?: ReactNode;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
  /** Stops outside clicks and Escape while work is in flight. */
  locked?: boolean;
}

export function Dialog({ open, onOpenChange, title, description, children, footer, width = 480, locked }: DialogProps) {
  const guard = (event: Event) => locked && event.preventDefault();
  return (
    <RadixDialog.Root open={open} onOpenChange={(next) => (locked ? undefined : onOpenChange(next))}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="arrive fixed inset-0 z-40 bg-[rgb(12_18_14/0.28)]" />
        <RadixDialog.Content
          tabIndex={-1}
          onOpenAutoFocus={(event) => {
            // Focus the field marked for it, or the dialog itself, rather than the close button.
            event.preventDefault();
            const content = event.currentTarget as HTMLElement;
            (content.querySelector<HTMLElement>("[data-autofocus]") ?? content).focus();
          }}
          onEscapeKeyDown={guard}
          onPointerDownOutside={guard}
          style={{ width }}
          className="rise fixed top-[12vh] left-1/2 z-50 flex max-h-[76vh] max-w-[calc(100vw-32px)] -translate-x-1/2 flex-col rounded-[14px] border border-rule bg-paper shadow-float focus:outline-none"
        >
          <div className="flex items-start gap-3 px-6 pt-5">
            <div className="min-w-0 flex-1">
              <RadixDialog.Title className="font-serif text-[20px] leading-tight font-semibold">{title}</RadixDialog.Title>
              {description ? (
                <RadixDialog.Description className="mt-1.5 text-[12.5px] leading-relaxed text-graphite">
                  {description}
                </RadixDialog.Description>
              ) : (
                <RadixDialog.Description className="sr-only">{title}</RadixDialog.Description>
              )}
            </div>
            <RadixDialog.Close asChild>
              <IconButton label="Close" className="-mt-1 -mr-2 size-7" disabled={locked}>
                <X size={15} />
              </IconButton>
            </RadixDialog.Close>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto px-6 pt-4 pb-5">{children}</div>
          {footer && <div className="flex items-center gap-2 border-t border-rule px-6 py-3.5">{footer}</div>}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}

/** A labelled line in a settings list: title and help on the left, a control on the right. */
export function SettingRow({ title, detail, children }: { title: ReactNode; detail?: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex items-start gap-4 py-3">
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-ink">{title}</div>
        {detail && <div className="mt-0.5 text-[12px] leading-relaxed text-graphite">{detail}</div>}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2 pt-0.5">{children}</div>}
    </div>
  );
}

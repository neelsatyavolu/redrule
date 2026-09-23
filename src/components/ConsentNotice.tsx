import { useEffect, useState } from "react";
import { api } from "../lib/api";
import { attempt } from "../lib/store";
import { Button } from "./ui";

const COPIED_MS = 2000;

/** Copies the recording notice from Settings, to paste into the call's chat. */
export function CopyNoticeButton({ variant = "quiet" }: { variant?: "quiet" | "ghost" }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), COPIED_MS);
    return () => window.clearTimeout(timer);
  }, [copied]);

  return (
    <Button
      size="sm"
      variant={variant}
      title="Copies a short message to paste into the call's chat"
      onClick={() => void attempt(api.copyConsentNotice).then((ok) => ok && setCopied(true))}
    >
      {copied ? "Copied" : "Copy notice"}
    </Button>
  );
}

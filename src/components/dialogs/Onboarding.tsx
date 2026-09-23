import { api } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import { canWriteNotes } from "../../lib/types";
import { Button, Dialog } from "../ui";
import { ApiKeyChoiceRow } from "./ApiKeyRows";
import { AccountRows, LocalNotesRow, PermissionRows } from "./SetupSections";

/** First launch: the two permissions and a way to write notes, in the order they are needed. */
export function Onboarding() {
  const app = useStore((s) => s.app);
  if (!app || app.settings.onboarded) return null;
  const ready = app.permissions.microphone && app.permissions.screenRecording && canWriteNotes(app);
  const finish = () => void attempt(() => api.updateSettings({ onboarded: true }));

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && finish()}
      title="Set up Redrule"
      width={560}
      description="Redrule records your calls, transcribes them on this Mac, and writes the notes with your own AI account or on this Mac. Recording other people can require their consent, so let them know."
      footer={
        <>
          <div className="flex-1" />
          <Button variant={ready ? "primary" : "ghost"} onClick={finish}>
            {ready ? "Start using Redrule" : "Finish later"}
          </Button>
        </>
      }
    >
      <Step number={1} title="Allow recording">
        <PermissionRows />
      </Step>
      <Step number={2} title="Choose how notes are written">
        <AccountRows />
        <div className="divide-y divide-rule border-t border-rule">
          <LocalNotesRow />
          <ApiKeyChoiceRow />
        </div>
      </Step>
    </Dialog>
  );
}

function Step({ number, title, children }: { number: number; title: string; children: React.ReactNode }) {
  return (
    <section className="mb-4 last:mb-0">
      <h3 className="flex items-center gap-2.5 text-[13px] font-semibold text-ink">
        <span className="grid size-5 place-items-center rounded-full bg-wash text-[11px] text-graphite tabular">{number}</span>
        {title}
      </h3>
      <div className="pl-[30px]">{children}</div>
    </section>
  );
}

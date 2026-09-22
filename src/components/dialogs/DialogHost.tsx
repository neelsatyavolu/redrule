import { useDialogs } from "./dialogState";
import { NoteEditor } from "./NoteEditor";
import { Onboarding } from "./Onboarding";
import { SettingsDialog } from "./SettingsDialog";
import { ShareDialog } from "./ShareDialog";
import { DeleteDialog, RenameDialog } from "./SimpleDialogs";

export function DialogHost() {
  const { current, close } = useDialogs();
  return (
    <>
      <Onboarding />
      {current?.kind === "rename" && <RenameDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "edit" && <NoteEditor meeting={current.meeting} onClose={close} />}
      {current?.kind === "share" && <ShareDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "delete" && <DeleteDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "settings" && <SettingsDialog initialTab={current.tab} onClose={close} />}
    </>
  );
}

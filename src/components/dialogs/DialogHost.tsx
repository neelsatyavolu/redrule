import { useDialogs } from "./dialogState";
import { NoteEditor } from "./NoteEditor";
import { Onboarding } from "./Onboarding";
import { SettingsDialog } from "./SettingsDialog";
import { ShareDialog } from "./ShareDialog";
import { ConfirmFolderDialog, JoinFolderDialog, MoveMeetingDialog, NewFolderDialog, RenameFolderDialog } from "./FolderDialogs";
import { DeleteDialog, RenameDialog } from "./SimpleDialogs";
import { TagsDialog } from "./TagsDialog";

export function DialogHost() {
  const { current, close } = useDialogs();
  return (
    <>
      <Onboarding />
      {current?.kind === "rename" && <RenameDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "tags" && <TagsDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "move" && <MoveMeetingDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "newFolder" && <NewFolderDialog onClose={close} />}
      {current?.kind === "joinFolder" && <JoinFolderDialog onClose={close} />}
      {current?.kind === "renameFolder" && <RenameFolderDialog folder={current.folder} onClose={close} />}
      {current?.kind === "resetFolder" && <ConfirmFolderDialog kind="reset" folder={current.folder} onClose={close} />}
      {current?.kind === "deleteFolder" && <ConfirmFolderDialog kind="delete" folder={current.folder} onClose={close} />}
      {current?.kind === "leaveFolder" && <ConfirmFolderDialog kind="leave" folder={current.folder} onClose={close} />}
      {current?.kind === "edit" && <NoteEditor meeting={current.meeting} onClose={close} />}
      {current?.kind === "share" && <ShareDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "delete" && <DeleteDialog meeting={current.meeting} onClose={close} />}
      {current?.kind === "settings" && <SettingsDialog initialTab={current.tab} onClose={close} />}
    </>
  );
}

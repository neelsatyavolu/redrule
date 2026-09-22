import { useState } from "react";
import { api } from "../../lib/api";
import { attempt } from "../../lib/store";
import type { Meeting } from "../../lib/types";
import { Button, Dialog, TextInput } from "../ui";

interface MeetingDialogProps {
  meeting: Meeting;
  onClose: () => void;
}

export function RenameDialog({ meeting, onClose }: MeetingDialogProps) {
  const [title, setTitle] = useState(meeting.title);
  const valid = title.trim() !== "";
  const save = async () => {
    if (valid && (await attempt(() => api.renameMeeting(meeting.id, title)))) onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(open) => !open && onClose()}
      title="Rename meeting"
      width={420}
      footer={
        <>
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button variant="primary" disabled={!valid} onClick={() => void save()}>
            Save
          </Button>
        </>
      }
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <TextInput data-autofocus aria-label="Title" value={title} maxLength={300} onChange={(e) => setTitle(e.target.value)} />
      </form>
    </Dialog>
  );
}

export function DeleteDialog({ meeting, onClose }: MeetingDialogProps) {
  const [busy, setBusy] = useState(false);
  const remove = async () => {
    setBusy(true);
    const deleted = await attempt(() => api.deleteMeeting(meeting.id), "Meeting deleted");
    setBusy(false);
    if (deleted) onClose();
  };

  return (
    <Dialog
      open
      locked={busy}
      onOpenChange={(open) => !open && onClose()}
      title="Delete this meeting?"
      width={440}
      description={`“${meeting.title}” will be removed from this Mac, with its notes and transcript. Any shared link stops working first. This can’t be undone.`}
      footer={
        <>
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="record" onClick={() => void remove()} disabled={busy}>
            {busy ? "Deleting…" : "Delete meeting"}
          </Button>
        </>
      }
    >
      {null}
    </Dialog>
  );
}

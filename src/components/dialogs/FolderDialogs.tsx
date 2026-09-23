import clsx from "clsx";
import { Check, Folder } from "lucide-react";
import { useState, type ReactNode } from "react";
import { api } from "../../lib/api";
import { attempt, folderScope, useStore } from "../../lib/store";
import type { FolderInfo, Meeting } from "../../lib/types";
import { Button, Dialog, TextInput } from "../ui";

/** Matches the backend's limits in crates/core/src/folders.rs. */
const MAX_NAME = 80;
const MAX_DISPLAY_NAME = 60;
const NO_FOLDERS: FolderInfo[] = [];

interface FormDialogProps {
  title: string;
  description?: ReactNode;
  action: string;
  busyAction: string;
  valid: boolean;
  /** Returns whether the dialog should close. */
  submit: () => Promise<boolean>;
  onClose: () => void;
  children: ReactNode;
}

/** A dialog around one form, locked while its request is in flight. */
function FormDialog({ title, description, action, busyAction, valid, submit, onClose, children }: FormDialogProps) {
  const [busy, setBusy] = useState(false);
  const run = async () => {
    if (!valid || busy) return;
    setBusy(true);
    const done = await submit();
    setBusy(false);
    if (done) onClose();
  };
  return (
    <Dialog
      open
      locked={busy}
      onOpenChange={(open) => !open && onClose()}
      title={title}
      description={description}
      width={440}
      footer={
        <>
          <div className="flex-1" />
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Cancel
          </Button>
          <Button variant="primary" disabled={!valid || busy} onClick={() => void run()}>
            {busy ? busyAction : action}
          </Button>
        </>
      }
    >
      <form
        className="space-y-3"
        onSubmit={(event) => {
          event.preventDefault();
          void run();
        }}
      >
        {children}
      </form>
    </Dialog>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="block">
      <span className="mb-1.5 block text-[12px] font-medium text-graphite">{label}</span>
      {children}
    </label>
  );
}

/** Opens the folder once it exists, so recording goes straight into it. */
function openFolder(id: string) {
  const { setLibrary, setTag, select } = useStore.getState();
  setLibrary(folderScope(id));
  setTag(null);
  select(null);
}

export function NewFolderDialog({ onClose }: { onClose: () => void }) {
  const displayName = useStore((s) => s.app?.displayName ?? "");
  const [name, setName] = useState("");
  const [yourName, setYourName] = useState(displayName);
  return (
    <FormDialog
      title="New shared folder"
      description="Anyone with the folder's link can read its meetings on the web, or add it to Redrule to see them there and record into it."
      action="Create and copy link"
      busyAction="Creating…"
      valid={name.trim() !== "" && yourName.trim() !== ""}
      onClose={onClose}
      submit={async () => {
        let id = "";
        const created = await attempt(async () => {
          id = await api.createFolder(name, yourName);
        }, "Folder created. Its link is copied.");
        if (created) openFolder(id);
        return created;
      }}
    >
      <Field label="Folder name">
        <TextInput data-autofocus value={name} maxLength={MAX_NAME} placeholder="Acme team" onChange={(e) => setName(e.target.value)} />
      </Field>
      <Field label="Your name, shown on meetings you add">
        <TextInput value={yourName} maxLength={MAX_DISPLAY_NAME} onChange={(e) => setYourName(e.target.value)} />
      </Field>
    </FormDialog>
  );
}

export function JoinFolderDialog({ onClose }: { onClose: () => void }) {
  const displayName = useStore((s) => s.app?.displayName ?? "");
  const [link, setLink] = useState("");
  const [yourName, setYourName] = useState(displayName);
  return (
    <FormDialog
      title="Join a shared folder"
      description="Paste the folder link someone sent you. You'll see everyone's meetings in it, and can record into it."
      action="Join folder"
      busyAction="Joining…"
      valid={link.trim() !== "" && yourName.trim() !== ""}
      onClose={onClose}
      submit={async () => {
        let id = "";
        const joined = await attempt(async () => {
          id = await api.joinFolder(link, yourName);
        }, "Joined the folder");
        if (joined) openFolder(id);
        return joined;
      }}
    >
      <Field label="Folder link">
        <TextInput data-autofocus value={link} placeholder="https://redrule.n3el.dev/f/…" onChange={(e) => setLink(e.target.value)} />
      </Field>
      <Field label="Your name, shown on meetings you add">
        <TextInput value={yourName} maxLength={MAX_DISPLAY_NAME} onChange={(e) => setYourName(e.target.value)} />
      </Field>
    </FormDialog>
  );
}

export function RenameFolderDialog({ folder, onClose }: { folder: FolderInfo; onClose: () => void }) {
  const [name, setName] = useState(folder.name);
  return (
    <FormDialog
      title="Rename folder"
      description="Everyone in the folder sees the new name."
      action="Save"
      busyAction="Saving…"
      valid={name.trim() !== ""}
      onClose={onClose}
      submit={() => attempt(() => api.renameFolder(folder.id, name))}
    >
      <TextInput data-autofocus aria-label="Folder name" value={name} maxLength={MAX_NAME} onChange={(e) => setName(e.target.value)} />
    </FormDialog>
  );
}

const CONFIRM = {
  reset: {
    title: "Reset this folder's link?",
    body: "The current link stops working for everyone, on the web and in Redrule. You get a new link to send to the people who should keep access.",
    action: "Reset and copy new link",
    busy: "Resetting…",
    // The folder moves to a new id; keep it open.
    run: async (id: string) => openFolder(await api.resetFolderLink(id)),
    done: "Link reset. The new link is copied.",
  },
  delete: {
    title: "Delete this folder?",
    body: "The folder and every meeting in it are removed for everyone. Meetings recorded on this Mac stay here. This can't be undone.",
    action: "Delete folder",
    busy: "Deleting…",
    run: (id: string) => api.deleteFolder(id),
    done: "Folder deleted",
  },
  leave: {
    title: "Leave this folder?",
    body: "Its meetings disappear from Redrule on this Mac. Meetings you added stay in the folder for the others and on this Mac.",
    action: "Leave folder",
    busy: "Leaving…",
    run: (id: string) => api.leaveFolder(id),
    done: "Left the folder",
  },
} as const;

export function ConfirmFolderDialog({ folder, kind, onClose }: { folder: FolderInfo; kind: keyof typeof CONFIRM; onClose: () => void }) {
  const copy = CONFIRM[kind];
  return (
    <FormDialog
      title={copy.title}
      description={`“${folder.name}”: ${copy.body}`}
      action={copy.action}
      busyAction={copy.busy}
      valid
      onClose={onClose}
      submit={() => attempt(() => copy.run(folder.id), copy.done)}
    >
      {null}
    </FormDialog>
  );
}

export function MoveMeetingDialog({ meeting, onClose }: { meeting: Meeting; onClose: () => void }) {
  const folders = useStore((s) => s.app?.folders ?? NO_FOLDERS);
  const [choice, setChoice] = useState<string | null>(meeting.folderId ?? null);
  const options: { id: string | null; label: string }[] = [
    { id: null, label: "Not in a folder" },
    ...folders.filter((f) => !f.unavailable).map((f) => ({ id: f.id, label: f.name })),
  ];
  return (
    <FormDialog
      title="Move to folder"
      description="Everyone in a shared folder can read its meetings, with notes and transcript. Audio stays on this Mac."
      action="Move"
      busyAction="Moving…"
      valid={choice !== (meeting.folderId ?? null)}
      onClose={onClose}
      submit={() => attempt(() => api.setMeetingFolder(meeting.id, choice))}
    >
      <div role="radiogroup" aria-label="Folder" className="space-y-0.5">
        {options.map((option) => (
          <button
            key={option.id ?? ""}
            type="button"
            role="radio"
            aria-checked={choice === option.id}
            onClick={() => setChoice(option.id)}
            className={clsx(
              "flex h-8 w-full items-center gap-2.5 rounded-[7px] px-2.5 text-left text-[13px]",
              choice === option.id ? "bg-wash text-ink" : "text-graphite hover:bg-wash hover:text-ink",
            )}
          >
            <Folder size={14} className={option.id ? "opacity-80" : "opacity-0"} aria-hidden />
            <span className="min-w-0 flex-1 truncate">{option.label}</span>
            {choice === option.id && <Check size={14} className="text-focus" aria-hidden />}
          </button>
        ))}
      </div>
    </FormDialog>
  );
}

import clsx from "clsx";
import { Archive, ArchiveRestore, Copy, FilePenLine, FolderOpen, Pencil, Share, Trash2 } from "lucide-react";
import { ContextMenu, DropdownMenu } from "radix-ui";
import type { ComponentType, ReactNode } from "react";
import { api } from "../lib/api";
import { attempt, canEdit, useStore } from "../lib/store";
import type { Meeting } from "../lib/types";
import { useDialogs } from "./dialogs/dialogState";

interface Item {
  label: string;
  icon: ComponentType<{ size?: number; className?: string }>;
  onSelect: () => void;
  disabled?: boolean;
  destructive?: boolean;
}

/** The actions for one meeting, shared by the sidebar's context menu and the toolbar's menu. */
function useMeetingItems(meeting: Meeting): Item[][] {
  const app = useStore((s) => s.app);
  const open = useDialogs((s) => s.open);
  const editable = canEdit(app, meeting);
  const done = meeting.status === "done";
  const busy = app?.sharingBusy ?? false;
  return [
    [
      { label: "Rename…", icon: Pencil, onSelect: () => open({ kind: "rename", meeting }), disabled: !editable },
      { label: "Edit notes…", icon: FilePenLine, onSelect: () => open({ kind: "edit", meeting }), disabled: !editable || !done },
    ],
    [
      { label: "Share…", icon: Share, onSelect: () => open({ kind: "share", meeting }), disabled: !done || busy },
      {
        label: "Copy as Markdown",
        icon: Copy,
        onSelect: () => void attempt(() => api.copyMarkdown(meeting.id), "Copied as Markdown"),
        disabled: !done,
      },
      { label: "Show in Finder", icon: FolderOpen, onSelect: () => void attempt(() => api.revealMeeting(meeting.id)) },
    ],
    [
      {
        label: meeting.archivedAt ? "Restore from archive" : "Archive",
        icon: meeting.archivedAt ? ArchiveRestore : Archive,
        onSelect: () =>
          void attempt(
            () => api.setArchived(meeting.id, !meeting.archivedAt),
            meeting.archivedAt ? "Restored from archive" : "Archived",
          ),
        disabled: !editable,
      },
      {
        label: "Delete…",
        icon: Trash2,
        onSelect: () => open({ kind: "delete", meeting }),
        disabled: !editable || busy,
        destructive: true,
      },
    ],
  ];
}

const CONTENT =
  "rise z-50 min-w-[210px] rounded-[10px] border border-rule bg-raised p-1 shadow-float focus:outline-none";
const ITEM =
  "flex h-7 items-center gap-2.5 rounded-[6px] px-2 text-[13px] text-ink outline-none data-[disabled]:opacity-35 data-[highlighted]:bg-focus data-[highlighted]:text-white";

function Items({ groups, Primitive }: { groups: Item[][]; Primitive: typeof ContextMenu | typeof DropdownMenu }) {
  return groups.map((group, index) => (
    <div key={group[0].label}>
      {index > 0 && <Primitive.Separator className="mx-2 my-1 h-px bg-rule" />}
      {group.map(({ label, icon: Icon, onSelect, disabled, destructive }) => (
        <Primitive.Item
          key={label}
          disabled={disabled}
          onSelect={onSelect}
          className={clsx(ITEM, destructive && "text-margin data-[highlighted]:bg-margin")}
        >
          <Icon size={14} className="opacity-80" />
          {label}
        </Primitive.Item>
      ))}
    </div>
  ));
}

export function MeetingContextMenu({ meeting, children }: { meeting: Meeting; children: ReactNode }) {
  const groups = useMeetingItems(meeting);
  return (
    <ContextMenu.Root>
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className={CONTENT}>
          <Items groups={groups} Primitive={ContextMenu} />
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}

export function MeetingDropdown({ meeting, children }: { meeting: Meeting; children: ReactNode }) {
  const groups = useMeetingItems(meeting);
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>{children}</DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={6} className={CONTENT}>
          <Items groups={groups} Primitive={DropdownMenu} />
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

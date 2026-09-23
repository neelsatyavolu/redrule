import clsx from "clsx";
import { Archive, Check, ChevronDown, Ellipsis, Folder, FolderPlus, Inbox, Link, LogOut, Pencil, RotateCcw, Trash2, UserPlus } from "lucide-react";
import { DropdownMenu } from "radix-ui";
import type { ComponentType } from "react";
import { api } from "../lib/api";
import { attempt, folderScope, scopeFolder, type Library } from "../lib/store";
import type { FolderInfo } from "../lib/types";
import { useDialogs } from "./dialogs/dialogState";
import { MENU_CONTENT, MENU_ITEM } from "./MeetingMenu";
import { IconButton } from "./ui";

type Icon = ComponentType<{ size?: number; className?: string }>;

function scopeLabel(library: Library, folders: FolderInfo[]): { label: string; icon: Icon } {
  const folder = scopeFolder(library);
  if (folder !== null) return { label: folders.find((f) => f.id === folder)?.name ?? "Folder", icon: Folder };
  return library === "archive" ? { label: "Archive", icon: Archive } : { label: "My meetings", icon: Inbox };
}

/** Chooses what the sidebar lists: this Mac's meetings, the archive, or a shared folder. */
export function ScopeMenu({ value, folders, onChange }: { value: Library; folders: FolderInfo[]; onChange: (next: Library) => void }) {
  const open = useDialogs((s) => s.open);
  const current = scopeLabel(value, folders);
  const choice = (library: Library, label: string, icon: Icon) => {
    const Icon = icon;
    return (
      <DropdownMenu.Item key={library} className={MENU_ITEM} onSelect={() => onChange(library)}>
        <Icon size={14} className="opacity-80" />
        <span className="min-w-0 flex-1 truncate">{label}</span>
        {value === library && <Check size={14} aria-label="Showing" />}
      </DropdownMenu.Item>
    );
  };
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          aria-label={`Showing ${current.label}. Change`}
          className="flex h-7 min-w-0 flex-1 items-center gap-2 rounded-[7px] bg-wash px-2 text-[12.5px] font-medium text-ink hover:bg-rule data-[state=open]:bg-rule"
        >
          <current.icon size={13} className="shrink-0 text-graphite" />
          <span className="min-w-0 flex-1 truncate text-left">{current.label}</span>
          <ChevronDown size={13} className="shrink-0 text-graphite" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="start" sideOffset={4} className={clsx(MENU_CONTENT, "max-h-[70vh] w-[236px] overflow-y-auto")}>
          {choice("meetings", "My meetings", Inbox)}
          {choice("archive", "Archive", Archive)}
          <DropdownMenu.Separator className="mx-2 my-1 h-px bg-rule" />
          <DropdownMenu.Label className="px-2 pt-1 pb-0.5 text-[11px] font-semibold text-faint">Shared folders</DropdownMenu.Label>
          {folders.map((folder) => choice(folderScope(folder.id), folder.name, Folder))}
          <DropdownMenu.Item className={MENU_ITEM} onSelect={() => open({ kind: "newFolder" })}>
            <FolderPlus size={14} className="opacity-80" />
            New folder…
          </DropdownMenu.Item>
          <DropdownMenu.Item className={MENU_ITEM} onSelect={() => open({ kind: "joinFolder" })}>
            <UserPlus size={14} className="opacity-80" />
            Join folder…
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

/** Copying the link, and the owner's rename, reset and delete; others can leave. */
export function FolderActions({ folder }: { folder: FolderInfo }) {
  const open = useDialogs((s) => s.open);
  const items: { label: string; icon: Icon; onSelect: () => void; destructive?: boolean }[] = [
    ...(folder.unavailable
      ? []
      : [{ label: "Copy link", icon: Link, onSelect: () => void attempt(() => api.copyFolderLink(folder.id), "Folder link copied") }]),
    ...(folder.owner && !folder.unavailable
      ? [
          { label: "Rename…", icon: Pencil, onSelect: () => open({ kind: "renameFolder", folder }) },
          { label: "Reset link…", icon: RotateCcw, onSelect: () => open({ kind: "resetFolder", folder }) },
          { label: "Delete folder…", icon: Trash2, onSelect: () => open({ kind: "deleteFolder", folder }), destructive: true },
        ]
      : [{ label: "Leave folder…", icon: LogOut, onSelect: () => open({ kind: "leaveFolder", folder }), destructive: true }]),
  ];
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <IconButton label="Folder actions" className="size-7 shrink-0">
          <Ellipsis size={15} />
        </IconButton>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={4} className={MENU_CONTENT}>
          {items.map(({ label, icon: Icon, onSelect, destructive }) => (
            <DropdownMenu.Item
              key={label}
              onSelect={onSelect}
              className={clsx(MENU_ITEM, destructive && "text-margin data-[highlighted]:bg-margin")}
            >
              <Icon size={14} className="opacity-80" />
              {label}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

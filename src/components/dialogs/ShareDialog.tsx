import { Link2 } from "lucide-react";
import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import { attempt, useStore } from "../../lib/store";
import type { Meeting, MeetingShare } from "../../lib/types";
import { Button, Dialog, Spinner, Switch } from "../ui";

export function ShareDialog({ meeting, onClose }: { meeting: Meeting; onClose: () => void }) {
  const busy = useStore((s) => s.app?.sharingBusy ?? false);
  const [share, setShare] = useState<MeetingShare | null>(null);
  const [includeTranscript, setIncludeTranscript] = useState(false);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    void api.meetingDetail(meeting.id).then(({ share }) => {
      setShare(share);
      if (share) setIncludeTranscript(share.includesTranscript);
    });
  }, [meeting.id]);

  const publish = async () => {
    let url = "";
    const published = await attempt(async () => {
      url = await api.publishShare(meeting.id, includeTranscript);
    });
    if (published) {
      setShare({ id: share?.id ?? "", url, includesTranscript: includeTranscript });
      setCopied(true);
    }
  };

  const stop = async () => {
    if (await attempt(() => api.revokeShare(meeting.id), "Sharing stopped")) {
      setShare(null);
      setCopied(false);
    }
  };

  const primaryLabel = copied ? "Link copied" : share ? "Update and copy link" : "Create and copy link";

  return (
    <Dialog
      open
      locked={busy}
      onOpenChange={(open) => !open && onClose()}
      title="Share notes"
      width={500}
      description="Anyone with the link can read a copy of these notes. Audio is never uploaded."
      footer={
        <>
          {share && (
            <Button variant="danger" onClick={() => void stop()} disabled={busy}>
              Stop sharing
            </Button>
          )}
          <div className="flex-1" />
          {busy && <Spinner />}
          <Button variant="ghost" onClick={onClose} disabled={busy}>
            Done
          </Button>
          <Button variant="primary" onClick={() => void publish()} disabled={busy}>
            {primaryLabel}
          </Button>
        </>
      }
    >
      <div className="rounded-[10px] border border-rule bg-raised">
        <div className="border-b border-rule px-4 py-3">
          <div className="truncate font-serif text-[15px] font-semibold">{meeting.title}</div>
        </div>
        <label className="flex items-center gap-4 px-4 py-3">
          <span className="flex-1">
            <span className="block text-[13px] font-medium">Include transcript</span>
            <span className="block text-[12px] text-graphite">Adds the full transcript with speaker names.</span>
          </span>
          <Switch
            label="Include transcript"
            checked={includeTranscript}
            onChange={(value) => {
              setIncludeTranscript(value);
              setCopied(false);
            }}
          />
        </label>
      </div>

      {share?.url && (
        <div className="mt-4 flex items-center gap-2.5 rounded-[10px] bg-wash px-3.5 py-2.5">
          <Link2 size={14} className="shrink-0 text-graphite" aria-hidden />
          <span className="selectable min-w-0 flex-1 truncate text-[12.5px] text-ink">{share.url}</span>
        </div>
      )}
      {share && (
        <p className="mt-3 text-[12px] leading-relaxed text-graphite">
          Edits and new speaker names reach the link when you update it. Stopping sharing disables the link, but copies
          someone already saved can’t be recalled.
        </p>
      )}
    </Dialog>
  );
}

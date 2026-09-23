import { useEffect, useState } from "react";
import { api } from "../../lib/api";
import type { Calendar } from "../../lib/types";
import { Switch } from "../ui";

/** The calendars whose events may name a recording, grouped by account. New calendars start on. */
export function CalendarChoices({ ignored, onChange }: { ignored: string[]; onChange: (ignored: string[]) => void }) {
  const [calendars, setCalendars] = useState<Calendar[] | null>(null);
  useEffect(() => {
    let current = true;
    void api.calendars().then((list) => current && setCalendars(list));
    return () => {
      current = false;
    };
  }, []);
  if (!calendars) return null;
  if (calendars.length === 0) {
    return <p className="pb-3 text-[12px] text-graphite">This Mac has no calendars yet.</p>;
  }

  const accounts = [...new Set(calendars.map((calendar) => calendar.account))];
  const toggle = (id: string, use: boolean) => onChange(use ? ignored.filter((other) => other !== id) : [...ignored, id]);
  const noneUsed = calendars.every((calendar) => ignored.includes(calendar.id));

  return (
    <div className="pb-3">
      {accounts.map((account) => (
        <div key={account} className="mt-2 first:mt-0">
          {account && <h4 className="text-[11.5px] font-semibold text-graphite">{account}</h4>}
          {calendars
            .filter((calendar) => calendar.account === account)
            .map((calendar) => (
              <div key={calendar.id} className="flex items-center justify-between gap-4 py-1">
                <span className="min-w-0 truncate text-[12.5px] text-ink">{calendar.title}</span>
                <Switch
                  label={`Use ${calendar.title}`}
                  checked={!ignored.includes(calendar.id)}
                  onChange={(use) => toggle(calendar.id, use)}
                />
              </div>
            ))}
        </div>
      ))}
      {noneUsed && <p className="mt-2 text-[12px] text-graphite">No calendar is on, so recordings keep their usual names.</p>}
    </div>
  );
}

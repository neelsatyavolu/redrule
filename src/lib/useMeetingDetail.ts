import { useEffect, useState } from "react";
import { api, errorMessage } from "./api";
import type { MeetingDetail } from "./types";

type DetailState = { status: "loading" } | { status: "ready"; detail: MeetingDetail } | { status: "error"; message: string };

/** Loads a meeting's transcript, notes and share link, reloading whenever the backend's revision changes. */
export function useMeetingDetail(id: string, revision: number): DetailState {
  const [state, setState] = useState<DetailState>({ status: "loading" });

  useEffect(() => {
    let current = true;
    api
      .meetingDetail(id)
      .then((detail) => current && setState({ status: "ready", detail }))
      .catch((error) => current && setState({ status: "error", message: errorMessage(error) }));
    return () => {
      current = false;
    };
  }, [id, revision]);

  // A different meeting starts from a clean slate rather than showing the previous one's content.
  const [shownId, setShownId] = useState(id);
  if (shownId !== id) {
    setShownId(id);
    setState({ status: "loading" });
  }
  return state;
}

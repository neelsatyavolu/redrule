import { getVersion } from "@tauri-apps/api/app";
import { useEffect, useState } from "react";
import { api, errorMessage } from "../../lib/api";
import { Button, SettingRow } from "../ui";

type Check =
  | { state: "idle" }
  | { state: "checking" }
  | { state: "latest" }
  | { state: "installed"; version: string }
  | { state: "failed"; message: string };

/** Checks for a new version on request. Redrule also checks by itself every few hours. */
export function UpdatesRow() {
  const [version, setVersion] = useState<string | null>(null);
  const [check, setCheck] = useState<Check>({ state: "idle" });
  useEffect(() => void getVersion().then(setVersion, () => setVersion(null)), []);

  const checkNow = async () => {
    setCheck({ state: "checking" });
    try {
      const installed = await api.checkForUpdates();
      setCheck(installed ? { state: "installed", version: installed } : { state: "latest" });
    } catch (error) {
      setCheck({ state: "failed", message: errorMessage(error) });
    }
  };

  return (
    <SettingRow title="Updates" detail={describe(check, version ? `Version ${version}` : "This version")}>
      {check.state === "installed" ? (
        <Button size="sm" onClick={() => void api.restartToUpdate()}>
          Restart
        </Button>
      ) : (
        <Button size="sm" disabled={check.state === "checking"} onClick={() => void checkNow()}>
          {check.state === "checking" ? "Checking…" : "Check for Updates"}
        </Button>
      )}
    </SettingRow>
  );
}

function describe(check: Check, current: string): string {
  switch (check.state) {
    case "idle":
      return `${current}. Redrule checks for new versions and installs them in the background.`;
    case "checking":
      return "Checking for a new version…";
    case "latest":
      return `${current} is the latest.`;
    case "installed":
      return `Redrule ${check.version} is installed. Restart to start using it.`;
    case "failed":
      return `Couldn’t check for updates. ${check.message}`;
  }
}

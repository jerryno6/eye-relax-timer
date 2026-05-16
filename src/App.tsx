import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Pause, Play, Square, Save, X, RefreshCcw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

type TimerStatus = "stopped" | "running" | "paused" | "breakVisible";

type AppSettings = {
  durationMinutes: number;
  repeatEnabled: boolean;
  autostartEnabled: boolean;
};

type TimerSnapshot = {
  status: TimerStatus;
  remainingSeconds: number;
  breakRemainingSeconds: number;
};

const PRESETS = [5, 10, 15, 20, 30, 45, 60];
const DEFAULT_SETTINGS: AppSettings = {
  durationMinutes: 20,
  repeatEnabled: true,
  autostartEnabled: false,
};
const DEFAULT_TIMER: TimerSnapshot = {
  status: "stopped",
  remainingSeconds: 20 * 60,
  breakRemainingSeconds: 0,
};

export function App() {
  const windowKind = new URLSearchParams(window.location.search).get("window");

  if (windowKind === "break") {
    return <BreakPopup />;
  }

  return <SettingsWindow />;
}

function SettingsWindow() {
  const [settings, setSettings] = useState<AppSettings>(DEFAULT_SETTINGS);
  const [timer, setTimer] = useState<TimerSnapshot>(DEFAULT_TIMER);
  const [customMinutes, setCustomMinutes] = useState(String(DEFAULT_SETTINGS.durationMinutes));
  const [saved, setSaved] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [updateState, setUpdateState] = useState<{
    status: "idle" | "checking" | "uptodate" | "outdated" | "error";
    message?: string;
    releaseUrl?: string;
  }>({ status: "idle" });

  useEffect(() => {
    void loadInitialState();

    async function loadInitialState() {
      try {
        const [loadedSettings, loadedTimer] = await Promise.all([
          invoke<AppSettings>("get_settings"),
          invoke<TimerSnapshot>("get_timer_state"),
        ]);
        setSettings(loadedSettings);
        setCustomMinutes(String(loadedSettings.durationMinutes));
        setTimer(loadedTimer);
      } catch (cause) {
        setError(String(cause));
      }
    }
  }, []);

  useEffect(() => {
    const unlisten = listen<TimerSnapshot>("timer-state", (event) => {
      setTimer(event.payload);
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  const selectedPreset = useMemo(
    () => PRESETS.includes(settings.durationMinutes),
    [settings.durationMinutes],
  );

  async function saveSettings(nextSettings = settings, showFeedback = true) {
    setError(null);
    setSaved(false);
    try {
      await invoke("save_settings", { settings: nextSettings });
      setSettings(nextSettings);
      setCustomMinutes(String(nextSettings.durationMinutes));
      if (showFeedback) {
        setSaved(true);
        window.setTimeout(() => setSaved(false), 1600);
      }
    } catch (cause) {
      setError(String(cause));
      throw cause;
    }
  }

  function updateDuration(value: number) {
    const next = { ...settings, durationMinutes: value };
    setSettings(next);
    setCustomMinutes(String(value));
  }

  function updateCustomDuration(value: string) {
    setCustomMinutes(value);
    const parsed = Number(value);
    if (Number.isInteger(parsed)) {
      setSettings((current) => ({ ...current, durationMinutes: parsed }));
    }
  }

  async function invokeTimer(command: "start_timer" | "pause_timer" | "stop_timer") {
    setError(null);
    try {
      if (command === "start_timer") {
        await saveSettings(settings, false);
      }
      await invoke(command);
    } catch (cause) {
      setError(String(cause));
    }
  }

  async function handleCheckUpdates() {
    setUpdateState({ status: "checking" });
    try {
      const r = await invoke<{
        current: string;
        latest: string;
        isOutdated: boolean;
        releaseUrl: string;
      }>("check_for_updates");
      if (r.isOutdated) {
        setUpdateState({ status: "outdated", message: `v${r.latest} available`, releaseUrl: r.releaseUrl });
      } else {
        setUpdateState({ status: "uptodate", message: `Up to date (v${r.current})` });
      }
    } catch (e) {
      setUpdateState({ status: "error", message: String(e) });
    }
  }

  const displayedTimer =
    timer.status === "stopped"
      ? { ...timer, remainingSeconds: settings.durationMinutes * 60 }
      : timer;
  const canPause = timer.status === "running" || timer.status === "paused";
  const pauseLabel = timer.status === "paused" ? "Resume" : "Pause";

  return (
    <main className="settings-shell">
      <section className="timer-overview" aria-label="Timer status">
        <p className="eyebrow">Eye Relax Timer</p>
        <div className="time-readout">{formatTimer(displayedTimer)}</div>
        <p className="status-line">{statusText(displayedTimer)}</p>
        <div className="control-row">
          <button className="icon-button primary" onClick={() => invokeTimer("start_timer")}>
            <Play size={18} />
            <span>Start</span>
          </button>
          <button
            className="icon-button"
            disabled={!canPause}
            onClick={() => invokeTimer("pause_timer")}
          >
            <Pause size={18} />
            <span>{pauseLabel}</span>
          </button>
          <button
            className="icon-button"
            disabled={timer.status === "stopped"}
            onClick={() => invokeTimer("stop_timer")}
          >
            <Square size={18} />
            <span>Stop</span>
          </button>
        </div>
      </section>

      <section className="settings-section" aria-label="Timer duration">
        <div className="section-heading">
          <h1>Duration</h1>
          <span>{settings.durationMinutes} min</span>
        </div>
        <div className="preset-grid">
          {PRESETS.map((minutes) => (
            <button
              className={settings.durationMinutes === minutes ? "preset active" : "preset"}
              key={minutes}
              onClick={() => updateDuration(minutes)}
            >
              {minutes}
            </button>
          ))}
        </div>
        <label className="field">
          <span>Custom minutes</span>
          <input
            min={1}
            max={240}
            type="number"
            value={customMinutes}
            onChange={(event) => updateCustomDuration(event.target.value)}
            className={selectedPreset ? undefined : "custom-active"}
          />
        </label>
      </section>

      <section className="settings-section" aria-label="Timer behavior">
        <label className="toggle-row">
          <span>
            <strong>Repeat timer</strong>
            <small>Start a new countdown after each break.</small>
          </span>
          <input
            type="checkbox"
            checked={settings.repeatEnabled}
            onChange={(event) =>
              setSettings((current) => ({ ...current, repeatEnabled: event.target.checked }))
            }
          />
        </label>
        <label className="toggle-row">
          <span>
            <strong>Open at login</strong>
            <small>Keep disabled until you opt in.</small>
          </span>
          <input
            type="checkbox"
            checked={settings.autostartEnabled}
            onChange={(event) =>
              setSettings((current) => ({ ...current, autostartEnabled: event.target.checked }))
            }
          />
        </label>
        <div className="toggle-row">
          <div className="update-check-group">
            <button
              className="icon-button"
              onClick={handleCheckUpdates}
              disabled={updateState.status === "checking"}
            >
              <RefreshCcw size={18} />
              <span>Check for updates</span>
            </button>
            {updateState.status === "outdated" && updateState.releaseUrl ? (
              <button
                className="update-status update-status--outdated"
                onClick={() => void openUrl(updateState.releaseUrl!)}
              >
                {updateState.message}
              </button>
            ) : updateState.message ? (
              <span
                className={`update-status update-status--${updateState.status === "uptodate" ? "ok" : "error"}`}
              >
                {updateState.message}
              </span>
            ) : null}
          </div>
        </div>
      </section>

      <footer className="footer-row">
        <div role="status" className={error ? "message error" : "message"}>
          {error ?? (saved ? "Saved" : "")}
        </div>
        <button className="icon-button primary" onClick={() => saveSettings()}>
          <Save size={18} />
          <span>Save</span>
        </button>
      </footer>
    </main>
  );
}

function BreakPopup() {
  const [timer, setTimer] = useState<TimerSnapshot>(DEFAULT_TIMER);

  useEffect(() => {
    void invoke<TimerSnapshot>("get_timer_state").then(setTimer);
    const unlisten = listen<TimerSnapshot>("timer-state", (event) => {
      setTimer(event.payload);
    });
    return () => {
      void unlisten.then((dispose) => dispose());
    };
  }, []);

  return (
    <main className="break-shell">
      <button className="close-button" onClick={() => invoke("close_break_popup")}>
        <X size={18} />
      </button>
      <section className="break-content" aria-label="Break timer">
        <p className="eyebrow">Rest your eyes</p>
        <h1>Look away from the screen</h1>
        <div className="break-count">{formatSeconds(timer.breakRemainingSeconds)}</div>
      </section>
    </main>
  );
}

function formatTimer(timer: TimerSnapshot) {
  if (timer.status === "breakVisible") {
    return formatSeconds(timer.breakRemainingSeconds);
  }
  return formatSeconds(timer.remainingSeconds);
}

function statusText(timer: TimerSnapshot) {
  switch (timer.status) {
    case "running":
      return "Running";
    case "paused":
      return "Paused";
    case "breakVisible":
      return "Break in progress";
    case "stopped":
      return "Ready";
  }
}

function formatSeconds(totalSeconds: number) {
  const safeSeconds = Math.max(0, Math.floor(totalSeconds));
  const minutes = Math.floor(safeSeconds / 60);
  const seconds = safeSeconds % 60;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

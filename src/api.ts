type Invoke<T = unknown> = (cmd: string, args?: Record<string, unknown>) => Promise<T>;

declare global {
  interface Window {
    __TAURI__?: {
      invoke: Invoke;
    };
  }
}

const invoke: Invoke = (cmd, args) => {
  return (window.__TAURI__ as any).invoke(cmd, args ?? {});
};

export { invoke };

export async function getSettings() {
  return invoke<any>("get_settings");
}

export async function saveSettings(settings: any) {
  return invoke<any>("save_settings", { settings });
}

export async function listVersions() {
  return invoke<any>("list_versions");
}

export async function launchGame(opts: {
  version: string;
  loader: string;
  offline: boolean;
  demo: boolean;
  dry_run?: boolean;
}) {
  return invoke<any>("launch_game", opts);
}

export async function beginLogin() {
  return invoke<any>("begin_login");
}

export async function pollLogin(deviceCode: string) {
  return invoke<any>("poll_login", { device_code: deviceCode });
}

export async function currentAccount() {
  return invoke<any>("current_account");
}

export async function logout() {
  return invoke<any>("logout");
}

export async function searchMods(query: string) {
  return invoke<any>("search_mods", { query });
}

export async function installMod(opts: {
  slug: string;
  mc_version: string;
  loader: string;
  instance: string;
}) {
  return invoke<any>("install_mod", opts);
}

export async function dataDir() {
  return invoke<any>("data_dir");
}

export function onProgress(cb: (done: number, total: number) => void) {
  const listener = (event: { payload: { done: number; total: number } }) => {
    cb(event.payload.done, event.payload.total);
  };
  (window.__TAURI__ as any).event.listen("progress", listener);
  return () => listener;
}

export function onLog(cb: (line: string) => void) {
  const listener = (event: { payload: string }) => {
    cb(event.payload);
  };
  (window.__TAURI__ as any).event.listen("launcher-log", listener);
  return () => listener;
}

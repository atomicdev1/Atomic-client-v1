import React, { useState, useEffect, useCallback, useRef } from "react";
import {
  getSettings,
  saveSettings,
  listVersions,
  launchGame,
  beginLogin,
  pollLogin,
  currentAccount,
  logout,
  searchMods,
  installMod,
  dataDir,
  onProgress,
  onLog,
} from "./api";
import "./styles.css";

type Version = { id: string; kind: string; release_time: string };

function App() {
  const [versions, setVersions] = useState<Version[]>([]);
  const [selected, setSelected] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [logs, setLogs] = useState<string[]>([]);
  const [progress, setProgress] = useState({ done: 0, total: 0 });
  const [account, setAccount] = useState<any>(null);
  const [settings, setSettings] = useState<any>({
    client_id: "",
    memory_mb: 4096,
    username: "Player",
    java_path: "",
    last_version: "",
    last_loader: "vanilla",
    instance: "default",
  } as any);
  const [loginStep, setLoginStep] = useState<"idle" | "code" | "polling">("idle");
  const [deviceCode, setDeviceCode] = useState<any>(null);
  const [modQuery, setModQuery] = useState("");
  const [modResults, setModResults] = useState<any[]>([]);
  const [modLoading, setModLoading] = useState(false);
  const [selectedLoader, setSelectedLoader] = useState("vanilla");
  const [selectedInstance, setSelectedInstance] = useState("default");

  const log = useCallback((line: string) => {
    setLogs((prev) => [...prev.slice(-500), line]);
  }, []);

  useEffect(() => {
    const p1 = onProgress((done, total) => setProgress({ done, total }));
    const p2 = onLog((line) => log(line));
    return () => { p1(); p2(); };
  }, [log]);

  useEffect(() => {
    (async () => {
      try {
        const s = await getSettings();
        setSettings(s);
        const v = await listVersions();
        setVersions(v.versions || v);
        const a = await currentAccount();
        if (a) setAccount(a);
      } catch (e: any) {
        log("ERR init: " + String(e?.message || e));
      }
    })();
  }, []);

  const handleInstall = useCallback(async () => {
    if (!selected) return;
    setLoading(true);
    try {
      const res = await launchGame({
        version: selected,
        loader: selectedLoader,
        offline: !account,
        demo: false,
        dry_run: true,
      });
      log("INSTALL ok: " + JSON.stringify(res));
    } catch (e: any) {
      log("ERR install: " + String(e?.message || e));
    } finally {
      setLoading(false);
    }
  }, [selected, selectedLoader, account, log]);

  const handleLaunch = useCallback(async () => {
    if (!selected) return;
    setLoading(true);
    try {
      const res = await launchGame({
        version: selected,
        loader: selectedLoader,
        offline: !account,
        demo: false,
        dry_run: false,
      });
      log("LAUNCH pid=" + res.pid + " instance=" + res.instance_dir);
    } catch (e: any) {
      log("ERR launch: " + String(e?.message || e));
    } finally {
      setLoading(false);
    }
  }, [selected, selectedLoader, account, log]);

  const handleBeginLogin = useCallback(async () => {
    try {
      const dc = await beginLogin();
      setDeviceCode(dc);
      setLoginStep("code");
      log("LOGIN code: " + dc.user_code + " -> " + dc.verification_uri);
    } catch (e: any) {
      log("ERR login: " + String(e?.message || e));
    }
  }, [log]);

  const handlePollLogin = useCallback(async () => {
    if (!deviceCode) return;
    setLoginStep("polling");
    try {
      const out = await pollLogin(deviceCode.device_code);
      if (out.pending) {
        setTimeout(() => { if (loginStep === "polling") handlePollLogin(); }, 3000);
        return;
      }
      setAccount(out.account);
      setLoginStep("idle");
      setDeviceCode(null);
      log("LOGGED IN as " + out.account.name);
    } catch (e: any) {
      log("ERR poll: " + String(e?.message || e));
      setLoginStep("idle");
    }
  }, [deviceCode, loginStep, handlePollLogin, log]);

  const handleSearch = useCallback(async () => {
    if (!modQuery.trim()) return;
    setModLoading(true);
    try {
      const hits = await searchMods(modQuery);
      setModResults(hits.hits || hits || []);
    } catch (e: any) {
      log("ERR search: " + String(e?.message || e));
    } finally {
      setModLoading(false);
    }
  }, [modQuery, log]);

  const handleInstallMod = useCallback(async (slug: string) => {
    try {
      const p = await installMod({ slug, mc_version: selected || "", loader: selectedLoader, instance: selectedInstance });
      log("MOD installed: " + p);
    } catch (e: any) {
      log("ERR mod install: " + String(e?.message || e));
    }
  }, [selected, selectedLoader, selectedInstance, log]);

  const pct = progress.total > 0 ? ((progress.done / progress.total) * 100).toFixed(0) : "0";

  return (
    <div className="app">
      <div className="sidebar">
        <div style={{ padding: 20, display: "flex", flexDirection: "column", gap: 6 }}>
          <div style={{ fontSize: 18, fontWeight: 700, letterSpacing: -0.5px }}>Atomic Client</div>
          <div style={{ fontSize: 11, color: "#8b93a5", textTransform: "uppercase", letterSpacing: 1 }}>launcher · v0.1.0</div>
        </div>

        <div className="card" style={{ padding: 16, margin: 16 }}>
          <h3>Account</h3>
          {account ? (
            <div className="field">
              <div style={{ minWidth: 100, color: "#8b93a5", fontSize: 12 }}>{account.name}</div>
              <button className="small danger" onClick={logout}>Sign out</button>
            </div>
          ) : (
            <button className="primary" style={{ width: "100%" }} onClick={handleBeginLogin} disabled={loginStep !== "idle"}>
              {loginStep === "polling" ? "Waiting for sign-in…" : "Sign in with Microsoft"}
            </button>
          )}
          {deviceCode && loginStep === "code" && (
            <div style={{ marginTop: 10, padding: 10, background: "#161822", borderRadius: 8, fontSize: 12, color: "#8b93a5" }}>
              Code: <b>{deviceCode.user_code}</b><br/>
              Go to: <b>{deviceCode.verification_uri}</b><br/>
              <button className="small" style={{ marginTop: 8, padding: 4, width: "100%" }} onClick={handlePollLogin}>I have authorized</button>
            </div>
          )}
        </div>

        <div className="card" style={{ padding: 16, margin: 16 }}>
          <h3>Version</h3>
          <ul className="version-list">
            {versions.map((v) => (
              <li key={v.id} onClick={() => setSelected(v.id)} className={selected === v.id ? "selected" : ""}>
                <div className="ver">{v.id}</div>
                <span className="kind">{v.kind}</span>
              </li>
            ))}
          </ul>
          {versions.length === 0 && <div className="empty">loading versions…</div>}
          <div style={{ marginTop: 8, fontSize: 12, color: "#8b93a5" }}>
            Selected: <b>{selected || "none"}</b>
          </div>
        </div>

        <div className="card" style={{ padding: 16, margin: 16 }}>
          <h3>Loader</h3>
          <div style={{ display: "flex", gap: 8 }}>
            {["vanilla", "fabric", "quilt"].map((l) => (
              <button key={l} className={selectedLoader === l ? "primary" : undefined} onClick={() => setSelectedLoader(l)} style={{ flex: 1 }}>{l}</button>
            ))}
          </div>
          <div style={{ marginTop: 8 }}>
            <label style={{ display: "block", fontSize: 12, color: "#8b93a5", marginBottom: 4 }}>Instance</label>
            <input value={selectedInstance} onChange={(e) => setSelectedInstance(e.target.value)} placeholder="default" />
          </div>
        </div>

        <div style={{ padding: 16, display: "flex", gap: 10 }}>
          <button className="primary" onClick={handleInstall} disabled={loading || !selected} style={{ flex: 1 }}>
            {loading ? "Preparing…" : "Install (dry-run)"}
          </button>
          <button className="primary" onClick={handleLaunch} disabled={loading || !selected} style={{ flex: 1 }}>
            {loading ? "Launching…" : "Launch"}
          </button>
        </div>
        <div style={{ padding: 0 16 16 16 }}>
          <div className="progress"><div className="bar" style={{ width: pct + "%" }} /></div>
          <div style={{ fontSize: 12, color: "#8b93a5", marginTop: 4 }}>{progress.done} / {progress.total} bytes ({pct}%)</div>
        </div>
      </div>

      <div className="main">
        <div className="card">
          <h3>Launcher log</h3>
          <div className="logs">
            {logs.map((l, i) => (
              <div key={i} className="line">{l}</div>
            ))}
          </div>
        </div>

        <div className="card">
          <h3>Settings</h3>
          <div className="row">
            <label>Azure client id</label>
            <input value={settings.client_id || ""} onChange={(e) => setSettings({ ...settings, client_id: e.target.value })} placeholder="optional" />
          </div>
          <div className="row">
            <label>Memory (MB)</label>
            <input type="number" value={settings.memory_mb || 4096} onChange={(e) => setSettings({ ...settings, memory_mb: Number(e.target.value) || 4096 })} />
          </div>
          <div className="row">
            <label>Offline username</label>
            <input value={settings.username || "Player"} onChange={(e) => setSettings({ ...settings, username: e.target.value })} />
          </div>
          <div className="row">
            <label>Java path</label>
            <input value={settings.java_path || ""} onChange={(e) => setSettings({ ...settings, java_path: e.target.value })} placeholder="auto (Mojang runtime)" />
          </div>
          <div className="row">
            <label>Instance</label>
            <input value={settings.instance || "default"} onChange={(e) => setSettings({ ...settings, instance: e.target.value })} />
          </div>
          <button className="primary small" style={{ marginTop: 10 }} onClick={async () => {
            try { await saveSettings(settings); setSettings(settings); log("settings saved"); } catch (e: any) { log("ERR save: " + String(e?.message || e)); }
          }}>Save settings</button>
        </div>

        <div className="card">
          <h3>Mods (Modrinth)</h3>
          <div style={{ display: "flex", gap: 8, marginBottom: 10 }}>
            <input value={modQuery} onChange={(e) => setModQuery(e.target.value)} placeholder="search mods…" />
            <button className="primary small" onClick={handleSearch} disabled={modLoading}>{modLoading ? "…" : "Search"}</button>
          </div>
          {modResults.map((m) => (
            <div key={m.slug} className="mod-result">
              <div className="info">
                <div className="slug">{m.slug}</div>
                <div className="desc">{m.title}</div>
              </div>
              <button className="small primary" onClick={() => handleInstallMod(m.slug)}>Add</button>
            </div>
          ))}
          {modResults.length === 0 && modQuery && <div className="empty">No results.</div>}
        </div>

        <div className="card">
          <h3>Data directory</h3>
          <p className="muted" style={{ marginTop: 4 }} id="data-dir">click below</p>
          <button className="small primary" onClick={async () => {
            try { const d: string = await dataDir(); const el = document.getElementById("data-dir"); if (el) el.textContent = d; log("data dir: " + d); } catch (e: any) { log("ERR dir: " + String(e?.message || e)); }
          }}>Show path</button>
        </div>
      </div>
    </div>
  );
}

export default App;

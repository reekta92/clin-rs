/**
 * main.js — Tauri adapter bootstrap
 *
 * Imported by index.html. Attaches globals so the clin_webview.html script
 * can reach them without import statements:
 *
 *   window.clinApi        — routes all API calls to Tauri IPC or fetch()
 *   window.clinNotify     — native OS notification (Tauri) or no-op (browser)
 *   window.clinPickVault  — native folder picker dialog (Tauri) or prompt()
 *   window.clinCopy       — clipboard write via Tauri plugin or navigator.clipboard
 */

import { api, notify, pickVaultDir, copyToClipboard } from "./tauri_adapter.js";

// Flag Tauri mode immediately so isTauri() returns true even before __TAURI_INTERNALS__ is checked
window.__CLIN_TAURI__ = true;

// Set clinApi immediately — this is what the boot IIFE polls for
window.clinApi       = api;
window.clinNotify    = notify;
window.clinPickVault = pickVaultDir;
window.clinCopy      = copyToClipboard;

// Window controls — dynamic import so a load failure doesn't block clinApi
import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
  const win = getCurrentWindow();
  window.clinWindow = {
    minimize:    () => win.minimize(),
    maximize:    () => win.toggleMaximize(),
    close:       () => win.close(),
    isMaximized: () => win.isMaximized(),
  };
}).catch(() => {
  window.clinWindow = null;
});

console.log(
  "[clin-tauri] adapter ready — running in",
  typeof window.__TAURI__ !== "undefined" ? "Tauri (native)" : "browser (fetch fallback)"
);

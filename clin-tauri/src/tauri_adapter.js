/**
 * tauri_adapter.js
 *
 * Maps HTTP-style API calls to Tauri IPC invoke() commands, with a plain
 * fetch() fallback for browser/dev-server use.
 *
 * Uses window.__TAURI_INTERNALS__.invoke directly — this is exactly what
 * @tauri-apps/api/core does internally, bypassing the module import chain.
 */

// ---------------------------------------------------------------------------
// Direct IPC via __TAURI_INTERNALS__ (injected by Tauri before any JS runs)
// ---------------------------------------------------------------------------

// __CLIN_TAURI__ is set synchronously by main.js — the reliable signal that
// we're running inside the Tauri process (no race with __TAURI_INTERNALS__).
function inTauri() {
  return !!window.__CLIN_TAURI__;
}

// Best-effort Tauri IPC for plugin commands (notify, dialog, clipboard).
// Falls back gracefully if __TAURI_INTERNALS__ is unavailable.
function tauriInvoke(cmd, args) {
  if (!window.__TAURI_INTERNALS__?.invoke) {
    return Promise.reject(new Error("__TAURI_INTERNALS__ not available"));
  }
  return window.__TAURI_INTERNALS__.invoke(cmd, args ?? {});
}

// Base URL of the embedded HTTP API server injected by Rust on startup.
// Falls back to a fixed default in case the eval races with page load.
function apiBase() {
  return window.__CLIN_API_URL__ || "http://127.0.0.1:7779";
}

// ---------------------------------------------------------------------------
// URL helpers
// ---------------------------------------------------------------------------

function getParam(endpoint, name) {
  const idx = endpoint.indexOf("?");
  if (idx === -1) return undefined;
  return new URLSearchParams(endpoint.slice(idx + 1)).get(name) ?? undefined;
}

function stripQuery(endpoint) {
  const idx = endpoint.indexOf("?");
  return idx === -1 ? endpoint : endpoint.slice(0, idx);
}

// ---------------------------------------------------------------------------
// Tauri IPC routing table
// ---------------------------------------------------------------------------

async function invokeCommand(invoke, endpoint, method, body) {
  const path = stripQuery(endpoint);
  const m = (method || "GET").toUpperCase();

  if (m === "GET") {
    switch (path) {
      case "/graph.json":        return invoke("get_graph");
      case "/tree":              return invoke("get_tree");
      case "/file":              return invoke("get_file",          { path: getParam(endpoint, "path") });
      case "/notes":             return invoke("get_notes");
      case "/notes/pinned":      return invoke("get_pinned_notes");
      case "/calendar":          return invoke("get_calendar");
      case "/properties":        return invoke("get_properties",    { path: getParam(endpoint, "path") });
      case "/outline":           return invoke("get_outline",       { path: getParam(endpoint, "path") });
      case "/subnotes":          return invoke("get_subnotes",      { path: getParam(endpoint, "path") });
      case "/bases":             return invoke("get_bases");
      case "/base":              return invoke("get_base",          { path: getParam(endpoint, "path") });
      case "/canvas-files":      return invoke("get_canvas_files");
      case "/canvas":            return invoke("get_canvas",        { path: getParam(endpoint, "path") });
      case "/themes":            return invoke("get_themes");
      case "/vaults":            return invoke("get_vaults");
      case "/templates":         return invoke("get_templates");
      case "/health":            return invoke("get_health");
      case "/settings":          return invoke("get_settings");
      case "/recent-vaults":     return invoke("get_recent_vaults");
      case "/search":            return invoke("search_notes",      { q: getParam(endpoint, "q") ?? "" });
      case "/smart-folder":      return invoke("get_smart_folder",  { name: getParam(endpoint, "name") ?? "" });
      default:
        throw new Error(`[clin-tauri] Unknown GET endpoint: ${endpoint}`);
    }
  }

  if (m === "POST") {
    switch (path) {
      case "/file/open":         return invoke("open_file",         { path: body?.path });
      case "/file/save":         return invoke("save_file",         body ?? {});
      case "/file/create":       return invoke("create_file",       body ?? {});
      case "/file/move":         return invoke("move_file",         body ?? {});
      case "/file/duplicate":    return invoke("duplicate_file",    body ?? {});
      case "/file/delete":       return invoke("delete_file",       body ?? {});
      case "/folder/create":     return invoke("create_folder",     body ?? {});
      case "/folder/delete":     return invoke("delete_folder",     body ?? {});
      case "/rename":            return invoke("rename_file",       body ?? {});
      case "/link/create":       return invoke("create_link",       body ?? {});
      case "/link/delete":       return invoke("delete_link",       body ?? {});
      case "/note/pin":          return invoke("pin_note",          body ?? {});
      case "/properties/save":   return invoke("save_properties",   body ?? {});
      case "/settings/theme":    return invoke("set_theme",         body ?? {});
      case "/settings/vault":    return invoke("set_vault",         body ?? {});
      case "/settings":          return invoke("set_settings",      body ?? {});
      case "/calendar/sync":     return invoke("sync_calendar",     body ?? {});
      case "/vault/pick":        return invoke("pick_vault_dir");
      default:
        throw new Error(`[clin-tauri] Unknown POST endpoint: ${endpoint}`);
    }
  }

  if (m === "DELETE") {
    switch (path) {
      case "/file/delete":       return invoke("delete_file",       body ?? {});
      case "/folder/delete":     return invoke("delete_folder",     body ?? {});
      default:
        throw new Error(`[clin-tauri] Unknown DELETE endpoint: ${endpoint}`);
    }
  }

  throw new Error(`[clin-tauri] Unsupported method "${method}" for ${endpoint}`);
}

// ---------------------------------------------------------------------------
// Browser fetch fallback
// ---------------------------------------------------------------------------

async function fetchFallback(endpoint, method, body) {
  const m = (method || "GET").toUpperCase();
  const init = { method: m, cache: "no-store" };
  if (m !== "GET" && m !== "HEAD" && body !== undefined) {
    init.headers = { "Content-Type": "application/json" };
    init.body = JSON.stringify(body);
  }
  const res = await fetch(endpoint, init);
  if (!res.ok) {
    const text = await res.text().catch(() => res.statusText);
    throw new Error(`[clin fetch] ${m} ${endpoint} → ${res.status}: ${text}`);
  }
  const ct = res.headers.get("content-type") ?? "";
  return ct.includes("application/json") ? res.json() : res.text();
}

// ---------------------------------------------------------------------------
// Tauri-native extras (only available inside Tauri — no-ops in browser)
// ---------------------------------------------------------------------------

export async function notify(title, body) {
  if (!inTauri()) return;
  try {
    await tauriInvoke("plugin:notification|notify", { title, body });
  } catch {}
}

export async function pickVaultDir() {
  if (inTauri()) {
    try {
      return await tauriInvoke("plugin:dialog|open", { directory: true, multiple: false, title: "Select vault directory" });
    } catch {}
  }
  return window.prompt("Enter vault path:");
}

export async function copyToClipboard(text) {
  if (inTauri()) {
    try {
      await tauriInvoke("plugin:clipboard-manager|write_text", { text });
      return;
    } catch {}
  }
  await navigator.clipboard.writeText(text).catch(() => {});
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * api(endpoint, method?, body?)
 *
 * In Tauri: routes through the custom clin:// URI scheme protocol handler,
 * which is handled synchronously in Rust (completely bypasses __TAURI_INTERNALS__
 * IPC, so it works regardless of injection timing).
 *
 * In browser: fetch("clin://...") throws TypeError (unknown scheme) → falls
 * back to fetchFallback() which hits the HTTP dev-server.
 */
export async function api(endpoint, method = "GET", body = undefined) {
  if (inTauri()) {
    return fetchFallback(apiBase() + endpoint, method, body);
  }
  return fetchFallback(endpoint, method, body);
}

export default api;

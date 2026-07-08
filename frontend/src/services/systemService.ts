/**
 * System Service
 *
 * Typed wrappers over the OS/shell-level Tauri commands (open a browser, a
 * Finder/Explorer window, or a system-settings pane).
 * Pure 1-to-1 wrappers over invoke() - no behavior changes vs. a direct invoke call.
 *
 * These command names were previously repeated as raw string literals across a
 * dozen components. Centralizing them means a backend rename breaks one file
 * instead of twelve, and each argument is typed.
 */

import { invoke } from '@tauri-apps/api/core';

/**
 * Open a URL in the user's default browser, outside the app webview.
 *
 * Errors propagate to the caller. Call sites differ in how they handle failure
 * (some `catch` and log, some fire-and-forget inside an `onClick`), so this
 * wrapper deliberately does NOT swallow errors - each caller keeps its current
 * behavior.
 */
export async function openExternalUrl(url: string): Promise<void> {
  await invoke('open_external_url', { url });
}

/**
 * Reveal the recordings folder in the OS file manager.
 */
export async function openRecordingsFolder(): Promise<void> {
  await invoke('open_recordings_folder');
}

/**
 * Reveal the folder holding the local SQLite database in the OS file manager.
 */
export async function openDatabaseFolder(): Promise<void> {
  await invoke('open_database_folder');
}

/**
 * Reveal the folder holding downloaded STT/LLM model files in the OS file manager.
 */
export async function openModelsFolder(): Promise<void> {
  await invoke('open_models_folder');
}

/**
 * The macOS privacy panes `open_system_settings` knows how to deep-link to.
 * Mirrors the `x-apple.systempreferences:...security?<pane>` suffixes the Rust
 * command builds (`src-tauri/src/utils.rs`).
 */
export type MacPreferencePane = 'Privacy_Microphone' | 'Privacy_ScreenCapture';

/**
 * Open a macOS System Settings privacy pane.
 *
 * **macOS only.** The underlying command is `#[cfg(target_os = "macos")]`-gated,
 * so on Windows/Linux it is not registered and this call rejects - callers are
 * expected to `catch` and fall back to instructing the user manually.
 *
 * `preferencePane` is REQUIRED by the Rust signature; omitting it makes Tauri
 * reject the call with an "invalid args" error even on macOS.
 */
export async function openSystemSettings(preferencePane: MacPreferencePane): Promise<void> {
  await invoke('open_system_settings', { preferencePane });
}

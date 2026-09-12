//! The panel window itself: create it, put it back where the user left it, set
//! its screen-share posture, and remember its position when it closes.
//!
//! ## What this window deliberately is not
//!
//! - **Not hidden from the taskbar or dock.** `skip_taskbar` is never called
//!   (ADR-0038 invariant 1). The panel is private — excluded from screen
//!   capture where the OS supports that — but the *application* is always
//!   visible to the person using the computer and to anyone looking at it.
//! - **Not transparent.** Transparency behaves inconsistently on Windows
//!   (tauri-apps/tauri#8308) and a panel that renders as an invisible rectangle
//!   is worse than an opaque one. The panel is a normal opaque surface with its
//!   own rounded chrome, drawn by the web layer.
//! - **Not a capture surface.** Nothing here starts audio, reads a transcript
//!   or calls a model. The window is a shell; I2 fills it with live context and
//!   I3 with insights.

use super::config::{
    CopilotConfig, PanelGeometry, DEFAULT_PANEL_HEIGHT, DEFAULT_PANEL_WIDTH, MIN_PANEL_HEIGHT,
    MIN_PANEL_WIDTH,
};
use super::policy::{self, ProtectionVerdict};
use super::store;
use tauri::{AppHandle, Manager, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

/// Window label. Also the identifier the `copilot` capability in
/// `tauri.conf.json` grants permissions to — keep the two in step.
pub const COPILOT_WINDOW_LABEL: &str = "copilot";

/// Route the panel loads from the static export, mirroring how the tray
/// navigates the main window (`window.location.assign('/settings')`).
const COPILOT_ROUTE: &str = "copilot";

/// Refused when a caller tries to open the panel while the copilot is off.
/// The command layer turns this into a message; the point is that the guard
/// lives here, so the window cannot be created by any path while disabled.
pub const DISABLED_ERROR: &str =
    "The live copilot is off. Turn it on in Settings → Copilot to open the panel.";

/// Is the panel currently open?
pub fn is_open<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.get_webview_window(COPILOT_WINDOW_LABEL).is_some()
}

/// Open the panel (or focus it if it is already open).
///
/// Guarded on the enable flag, so this is also the structural answer to "can a
/// disabled copilot still put a window on screen?" — it cannot.
pub fn open<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let config = store::load_config(app);
    open_with(app, &config)
}

fn open_with<R: Runtime>(app: &AppHandle<R>, config: &CopilotConfig) -> Result<(), String> {
    if !config.enabled {
        return Err(DISABLED_ERROR.to_string());
    }

    if let Some(window) = app.get_webview_window(COPILOT_WINDOW_LABEL) {
        window
            .show()
            .map_err(|e| format!("Could not show the copilot panel: {e}"))?;
        let _ = window.set_focus();
        return Ok(());
    }

    let geometry = store::load_geometry(app);
    let (width, height) = geometry
        .map(|g| (g.width, g.height))
        .unwrap_or((DEFAULT_PANEL_WIDTH, DEFAULT_PANEL_HEIGHT));

    let mut builder = WebviewWindowBuilder::new(
        app,
        COPILOT_WINDOW_LABEL,
        WebviewUrl::App(COPILOT_ROUTE.into()),
    )
    .title("Mityu copilot")
    .inner_size(width, height)
    .min_inner_size(MIN_PANEL_WIDTH, MIN_PANEL_HEIGHT)
    .resizable(true)
    .decorations(false)
    .always_on_top(true)
    // Double-clicking a `data-tauri-drag-region` sends `internal_toggle_maximize`
    // (tauri 2.11.1 `src/window/scripts/drag.js`), and no attribute value keeps
    // dragging while disabling that — so the header would otherwise maximize a
    // panel that is meant to sit beside a meeting window. Documented
    // **Unsupported on Linux**, which is why `current_geometry` refuses a
    // maximized frame as well rather than trusting this alone.
    .maximizable(false)
    // Built hidden: content protection is applied before the first frame is
    // shown, so the panel cannot appear in a capture for the frame between
    // creation and the protection call.
    .visible(false);

    if let Some(g) = geometry {
        builder = builder.position(g.x, g.y);
    }

    let window = builder
        .build()
        .map_err(|e| format!("Could not open the copilot panel: {e}"))?;

    apply_content_protection(&window, config.content_protection);
    remember_position_on_close(app, &window);

    window
        .show()
        .map_err(|e| format!("Could not show the copilot panel: {e}"))?;
    let _ = window.set_focus();
    Ok(())
}

/// Close the panel, remembering where it was. A no-op when it is not open.
pub fn close<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let Some(window) = app.get_webview_window(COPILOT_WINDOW_LABEL) else {
        return Ok(());
    };
    persist_geometry(app, &window);
    window
        .close()
        .map_err(|e| format!("Could not close the copilot panel: {e}"))
}

/// Show it if hidden, hide it if shown — what the global shortcut and the tray
/// entry both do.
pub fn toggle<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    match app.get_webview_window(COPILOT_WINDOW_LABEL) {
        Some(window) if window.is_visible().unwrap_or(false) => close(app),
        Some(window) => {
            window
                .show()
                .map_err(|e| format!("Could not show the copilot panel: {e}"))?;
            let _ = window.set_focus();
            Ok(())
        }
        None => open(app),
    }
}

/// Push the current content-protection setting to an open panel and report what
/// the platform will actually do with it. Returns the verdict even when the
/// panel is closed, because Settings shows the sentence either way.
pub fn refresh_content_protection<R: Runtime>(
    app: &AppHandle<R>,
    requested: bool,
) -> ProtectionVerdict {
    match app.get_webview_window(COPILOT_WINDOW_LABEL) {
        Some(window) => apply_content_protection(&window, requested),
        None => policy::current_verdict(),
    }
}

fn apply_content_protection<R: Runtime>(
    window: &WebviewWindow<R>,
    requested: bool,
) -> ProtectionVerdict {
    let verdict = policy::current_verdict();
    if verdict.should_request() {
        if let Err(e) = window.set_content_protected(requested) {
            // Logged, not surfaced as a failure: the panel still works, and the
            // verdict the UI shows is about the platform, not about this call.
            log::warn!("copilot: the platform refused the content-protection request: {e}");
        }
    } else if requested {
        log::debug!(
            "copilot: content protection requested but this platform has no mechanism for it"
        );
    }
    verdict
}

/// Persist the panel's position when the user closes it themselves (the window
/// chrome's close button, `Alt+F4`, the shortcut). The programmatic paths call
/// [`persist_geometry`] directly before closing.
fn remember_position_on_close<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let app = app.clone();
    // The handle is looked up again inside the closure rather than captured, so
    // the window does not hold a clone of itself in its own event handler.
    window.on_window_event(move |event| {
        if matches!(event, tauri::WindowEvent::CloseRequested { .. }) {
            if let Some(window) = app.get_webview_window(COPILOT_WINDOW_LABEL) {
                persist_geometry(&app, &window);
            }
        }
    });
}

fn persist_geometry<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    if let Some(geometry) = current_geometry(window) {
        store::save_geometry(app, geometry);
    }
}

/// Read the panel's logical position and size. Physical pixels are converted
/// with the window's own scale factor, so a panel placed on a 200 % display
/// reopens the same size rather than half or double it.
///
/// The decision of what is worth storing — in particular that a maximized frame
/// never is — lives in [`PanelGeometry::from_window_metrics`], where it can be
/// tested without a window.
///
/// **Known limitation (Windows, mixed DPI):** the round-trip is logical, and on
/// Windows tao resolves a logical position by scanning monitors in enumeration
/// order, so a panel left on a 200 % external display can reopen on a 100 %
/// primary one. Storing physical pixels instead would break macOS, where
/// `set_outer_position` re-applies the window's own scale factor; fixing it
/// properly means versioning the stored key and converting against the target
/// monitor. It is cosmetic and self-correcting (the next close stores the
/// corrected position), so it is left for a follow-up rather than risking the
/// platform that works.
fn current_geometry<R: Runtime>(window: &WebviewWindow<R>) -> Option<PanelGeometry> {
    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    PanelGeometry::from_window_metrics(
        window.is_maximized().unwrap_or(false),
        position.x,
        position.y,
        size.width,
        size.height,
    )
}

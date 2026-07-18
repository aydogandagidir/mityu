import packageManifest from '../../package.json';

// Browser renders cannot call Tauri's getVersion(). Keep their fallback tied to
// the package manifest, which the release workflow checks against Tauri/Cargo.
export const APP_VERSION = packageManifest.version;

// TypeScript type definitions for system audio functionality

// NOTE: these are detection commands only — none of them capture audio.
// The capture-flavoured commands this file used to declare
// (startSystemAudioCaptureCommand, listSystemAudioDevicesCommand,
// checkSystemAudioPermissionsCommand) wrapped a dead Rust surface that has been
// removed; see audio/capture/mod.rs. Permission checks go through the
// audio::permissions::* commands instead.
export interface SystemAudioCommands {
  // Start monitoring system audio usage by other applications
  startSystemAudioMonitoring(): Promise<void>;

  // Stop monitoring system audio usage
  stopSystemAudioMonitoring(): Promise<void>;

  // Get the current status of system audio monitoring
  getSystemAudioMonitoringStatus(): Promise<boolean>;
}

// Event types emitted by the system audio detector
export interface SystemAudioEvents {
  'system-audio-started': string[]; // Array of app names using system audio
  'system-audio-stopped': void;
}

// Example usage in React component:
/*
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

// Start monitoring system audio
await invoke('start_system_audio_monitoring');

// Listen for system audio events
const unlisten = await listen<string[]>('system-audio-started', (event) => {
  console.log('Apps using system audio:', event.payload);
});

// Stop monitoring when component unmounts
await invoke('stop_system_audio_monitoring');
unlisten();
*/
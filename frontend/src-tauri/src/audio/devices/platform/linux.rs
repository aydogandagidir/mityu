use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait};

use crate::audio::devices::configuration::{AudioDevice, DeviceType};

/// Configure Linux audio devices using ALSA — **microphone (input) capture only**.
///
/// System-audio capture is not supported on Linux (ADR-0022). This function used to
/// scan the ALSA host for names containing `"monitor"` and register them as
/// `"<name> (System Audio)"` output devices. That was doubly broken: monitor sources
/// are a PulseAudio/PipeWire concept the ALSA host does not surface, and the suffixed
/// name could never round-trip — `AudioDevice::from_name` hard-errors on any name not
/// ending in `(input)`/`(output)`, and `get_device_and_config` matches raw cpal names
/// exactly. The entries only cluttered the device picker with options that always
/// failed. Re-adding them requires a real PulseAudio/PipeWire capture backend.
pub fn configure_linux_audio(host: &cpal::Host) -> Result<Vec<AudioDevice>> {
    let mut devices = Vec::new();

    // Add input devices
    for device in host.input_devices()? {
        if let Ok(name) = device.name() {
            devices.push(AudioDevice::new(name, DeviceType::Input));
        }
    }

    Ok(devices)
}

//! Enumeração de dispositivos de áudio (CoreAudio, WASAPI ou ALSA, via cpal).

pub use crate::platform::{is_virtual, virtual_output};

use cpal::traits::{DeviceTrait, HostTrait};
use cpal::{Device, Host};

pub fn input_names() -> Vec<String> {
    cpal::default_host()
        .input_devices()
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

pub fn output_names() -> Vec<String> {
    cpal::default_host()
        .output_devices()
        .map(|ds| ds.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default()
}

pub fn find_input(host: &Host, name: Option<&str>) -> Option<Device> {
    match name {
        Some(n) => host.input_devices().ok()?.find(|d| d.name().is_ok_and(|x| x == n)),
        None => host.default_input_device(),
    }
}

pub fn find_output(host: &Host, name: Option<&str>) -> Option<Device> {
    match name {
        Some(n) => host.output_devices().ok()?.find(|d| d.name().is_ok_and(|x| x == n)),
        None => host.default_output_device(),
    }
}

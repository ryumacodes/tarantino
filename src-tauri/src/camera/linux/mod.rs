//! Discover usable camera sources rather than every /dev/video* node.
use anyhow::{Context, Result};
use gst::prelude::*;

pub fn enumerate_camera_devices() -> Result<Vec<crate::state::AudioDevice>> {
    gst::init().context("Failed to initialize camera discovery")?;
    // V4L2 also exposes metadata and output nodes. Its provider probes their
    // capabilities without starting camera streams or unrelated backends.
    let provider = gst::DeviceProviderFactory::find("v4l2deviceprovider")
        .and_then(|factory| factory.get())
        .context("The GStreamer V4L2 camera discovery provider is missing")?;
    // devices() performs a one-shot probe; each picker refresh probes again.
    let mut devices = Vec::new();
    for device in provider.devices() {
        let Ok(source) = device.create_element(None) else {
            continue;
        };
        // Keep V4L2 identifiers compatible with the existing camera selection
        // contract. Other providers may expose screen or network sources.
        if source
            .factory()
            .is_none_or(|factory| factory.name() != "v4l2src")
        {
            continue;
        }
        let id = source.property::<String>("device");
        devices.push(crate::state::AudioDevice {
            id,
            name: device.display_name().to_string(),
            is_default: false,
        });
    }
    devices.sort_by(|a, b| a.id.cmp(&b.id));
    devices.dedup_by(|a, b| a.id == b.id);
    // video0 is not necessarily a camera (it may be a metadata/codec node).
    if let Some(first) = devices.first_mut() {
        first.is_default = true;
    }
    Ok(devices)
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "requires the native GStreamer V4L2 device provider"]
    fn linux_camera_discovery_smoke() {
        let devices = super::enumerate_camera_devices().unwrap();
        assert_eq!(
            devices.iter().filter(|d| d.is_default).count(),
            usize::from(!devices.is_empty())
        );
        for device in &devices {
            assert!(device.id.starts_with("/dev/"));
            assert!(!device.name.is_empty());
        }
        eprintln!("Discovered {} video capture sources", devices.len());
    }
}

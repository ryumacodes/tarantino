//! Linux microphone discovery and selection through the same backend used to record.
use crate::state::{AudioDevice, AudioDevices};
use cpal::traits::{DeviceTrait, HostTrait};

pub fn devices() -> Result<AudioDevices, String> {
    let host = cpal::default_host();
    let mut microphones = Vec::new();
    if host
        .default_input_device()
        .is_some_and(|device| device.default_input_config().is_ok())
    {
        microphones.push(AudioDevice {
            id: "default".into(),
            name: "System Default".into(),
            is_default: true,
        });
    }
    for device in host.input_devices().map_err(|error| error.to_string())? {
        // ALSA hints can include playback-only entries and unconfigured plugins.
        if device.default_input_config().is_err() {
            continue;
        }
        let Ok(id) = device.id() else { continue };
        let id = id.to_string();
        if microphones.iter().any(|device| device.id == id) {
            continue;
        }
        microphones.push(AudioDevice {
            id,
            name: device.to_string(),
            is_default: false,
        });
    }
    Ok(AudioDevices {
        microphones,
        system_sources: vec![AudioDevice {
            id: "default".into(),
            name: "System Output".into(),
            is_default: true,
        }],
    })
}

pub fn microphone(id: Option<&str>) -> Result<cpal::Device, String> {
    let host = cpal::default_host();
    let device = match id.filter(|id| !id.is_empty() && *id != "default") {
        None => host
            .default_input_device()
            .ok_or_else(|| "No default microphone input device available".to_string()),
        Some(id) => {
            let parsed = id
                .parse::<cpal::DeviceId>()
                .map_err(|error| format!("Invalid microphone device: {error}"))?;
            // Resolve through input enumeration: ALSA's direct lookup does not
            // round-trip every identifier returned for card-specific devices.
            host.input_devices()
                .map_err(|error| error.to_string())?
                .find(|device| device.id().ok().as_ref() == Some(&parsed))
                .ok_or_else(|| {
                    "Selected microphone is unavailable; choose another input".to_string()
                })
        }
    }?;
    device
        .default_input_config()
        .map_err(|error| format!("Microphone input is unavailable: {error}"))?;
    Ok(device)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires ALSA null input; records synthetic silence only"]
    async fn linux_microphone_wav_smoke() {
        use std::io::Read;
        use std::sync::Arc;
        use tokio::sync::Mutex;

        // Never fall back to a real microphone when the synthetic device is absent.
        let id = cpal::default_host()
            .input_devices()
            .unwrap()
            .filter_map(|device| device.id().ok())
            .find(|id| id.id() == "null")
            .expect("ALSA null input is required")
            .to_string();
        let path = std::env::temp_dir().join(format!("tarantino-mic-{}.wav", uuid::Uuid::new_v4()));
        let stop = Arc::new(Mutex::new(false));
        let task = super::super::spawn_microphone_task(path.clone(), Some(id), Arc::clone(&stop));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        *stop.lock().await = true;
        let result = task.await;
        let contents = (|| -> std::io::Result<([u8; 44], u64)> {
            let mut file = std::fs::File::open(&path)?;
            let mut header = [0; 44];
            file.read_exact(&mut header)?;
            Ok((header, file.metadata()?.len()))
        })();
        let _ = std::fs::remove_file(&path);
        result
            .expect("microphone task panicked")
            .expect("capture synthetic input");
        let (header, length) = contents.expect("read finalized WAV");
        assert_eq!(&header[0..4], b"RIFF");
        assert_eq!(&header[8..12], b"WAVE");
        assert_eq!(&header[36..40], b"data");
        assert!(length > 44, "no microphone samples were written");
        assert_eq!(
            u32::from_le_bytes(header[4..8].try_into().unwrap()) as u64 + 8,
            length
        );
        assert_eq!(
            u32::from_le_bytes(header[40..44].try_into().unwrap()) as u64 + 44,
            length
        );
        assert_eq!(u16::from_le_bytes(header[34..36].try_into().unwrap()), 16);
    }

    #[test]
    fn invalid_microphone_does_not_fall_back_to_default() {
        assert!(microphone(Some("invalid-device-id")).is_err());
    }

    #[test]
    #[ignore = "requires a desktop audio backend; does not record audio"]
    fn linux_microphone_device_selection_smoke() {
        let devices = devices().expect("enumerate microphones");
        assert!(
            !devices.microphones.is_empty(),
            "no inputs available on this host"
        );
        for listed in devices.microphones {
            let device = microphone(Some(&listed.id)).expect("resolve listed input");
            if listed.id != "default" {
                assert_eq!(device.id().unwrap().to_string(), listed.id);
            }
            println!("Resolved microphone: {}", listed.name);
        }
    }
}

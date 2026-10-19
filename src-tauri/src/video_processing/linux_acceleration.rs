//! Linux hardware selection; probe every render node before software fallback.
use super::ExportAcceleration;
use std::path::PathBuf;

pub(super) fn select_linux_acceleration(
    requested: &str,
    codec: &str,
    encoders: &str,
    va_devices: Vec<PathBuf>,
    nvidia_device: bool,
    mut verify: impl FnMut(&ExportAcceleration) -> bool,
) -> ExportAcceleration {
    let has = |name: &str| encoders.split_whitespace().any(|item| item == name);
    let nvenc = if codec == "h265" {
        "hevc_nvenc"
    } else {
        "h264_nvenc"
    };
    let vaapi = if codec == "h265" {
        "hevc_vaapi"
    } else {
        "h264_vaapi"
    };
    let amf = if codec == "h265" {
        "hevc_amf"
    } else {
        "h264_amf"
    };

    let mut choose = |backend: &str| {
        if backend == "vaapi" && has(vaapi) {
            // Render-node numbering is not a capability ranking. Test every
            // candidate before giving up on hardware encoding.
            return va_devices.iter().find_map(|device| {
                let candidate = ExportAcceleration::Vaapi {
                    device: device.clone(),
                };
                verify(&candidate).then_some(candidate)
            });
        }
        match backend {
            "nvenc" | "nvidia" if nvidia_device && has(nvenc) => Some(ExportAcceleration::Nvidia),
            "amf" if !va_devices.is_empty() && has(amf) => Some(ExportAcceleration::AmdAmf),
            "software" => Some(ExportAcceleration::Software),
            _ => None,
        }
        .filter(|candidate| verify(candidate))
    };

    if requested != "auto" {
        if let Some(backend) = choose(&requested) {
            return backend;
        }
        eprintln!(
            "Requested Linux video backend '{requested}' is unavailable; selecting automatically"
        );
    }
    for backend in ["nvenc", "vaapi", "amf"] {
        if backend == requested || (backend == "nvenc" && requested == "nvidia") {
            continue;
        }
        if let Some(selected) = choose(backend) {
            return selected;
        }
    }
    ExportAcceleration::Software
}

#[cfg(test)]
mod tests {
    use super::*;

    fn devices() -> Vec<PathBuf> {
        ["/dev/dri/renderD128", "/dev/dri/renderD129"]
            .map(PathBuf::from)
            .to_vec()
    }

    #[test]
    fn failed_first_gpu_does_not_force_software() {
        let mut probed = Vec::new();
        let selected = select_linux_acceleration(
            "auto",
            "h264",
            "h264_vaapi",
            devices(),
            false,
            |candidate| {
                probed.push(candidate.clone());
                matches!(candidate, ExportAcceleration::Vaapi { device } if device.ends_with("renderD129"))
            },
        );
        assert_eq!(probed.len(), 2);
        assert_eq!(
            selected,
            ExportAcceleration::Vaapi {
                device: devices()[1].clone()
            }
        );
    }

    #[test]
    fn explicit_vaapi_also_checks_all_gpus() {
        let selected = select_linux_acceleration(
            "vaapi",
            "h265",
            "hevc_nvenc hevc_vaapi",
            devices(),
            true,
            |candidate| matches!(candidate, ExportAcceleration::Vaapi { device } if device.ends_with("renderD129")),
        );
        assert_eq!(
            selected,
            ExportAcceleration::Vaapi {
                device: devices()[1].clone()
            }
        );
    }

    #[test]
    fn software_request_never_probes_hardware() {
        let selected = select_linux_acceleration(
            "software",
            "h264",
            "h264_nvenc h264_vaapi",
            devices(),
            true,
            |candidate| {
                assert_eq!(*candidate, ExportAcceleration::Software);
                true
            },
        );
        assert_eq!(selected, ExportAcceleration::Software);
    }

    #[test]
    fn unsupported_codec_skips_device_probes() {
        let selected =
            select_linux_acceleration("auto", "h265", "h264_vaapi", devices(), false, |_| {
                panic!("H.264 support does not establish H.265 support")
            });
        assert_eq!(selected, ExportAcceleration::Software);
    }

    #[test]
    fn all_unusable_devices_fall_back_to_software() {
        let selected =
            select_linux_acceleration("auto", "h264", "h264_vaapi", devices(), false, |_| false);
        assert_eq!(selected, ExportAcceleration::Software);
    }
}

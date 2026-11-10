use std::io::{self, Write};

const CHECK_INTERVAL: usize = 64 * 1024;

struct ResponseWriter<'a> {
    bytes: Vec<u8>,
    next_check: usize,
    check: &'a dyn Fn() -> anyhow::Result<()>,
}

impl Write for ResponseWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.bytes.len() >= self.next_check {
            (self.check)().map_err(io::Error::other)?;
            self.next_check = self.bytes.len().saturating_add(CHECK_INTERVAL);
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

pub(super) fn serialize(
    value: &impl serde::Serialize,
    check: Option<&dyn Fn() -> anyhow::Result<()>>,
) -> Result<String, String> {
    let Some(check) = check else {
        return serde_json::to_string(value).map_err(|error| error.to_string());
    };
    let mut writer = ResponseWriter { bytes: Vec::new(), next_check: 0, check };
    serde_json::to_writer(&mut writer, value).map_err(|error| error.to_string())?;
    check().map_err(|error| error.to_string())?;
    String::from_utf8(writer.bytes).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn cancellation_interrupts_a_large_response_before_serialization_finishes() {
        let checks = Cell::new(0);
        let check = || {
            checks.set(checks.get() + 1);
            anyhow::ensure!(checks.get() < 3, "Cursor preview cancelled");
            Ok(())
        };
        let frames = vec![[0.12345679_f32; 10]; 100_000];
        let error = serialize(&frames, Some(&check)).unwrap_err();
        assert!(error.contains("Cursor preview cancelled"));
        assert_eq!(checks.get(), 3);
    }

    #[test]
    fn checked_serialization_preserves_exact_response_bytes() {
        let checks = Cell::new(0);
        let check = || { checks.set(checks.get() + 1); Ok(()) };
        let frames = vec![[0.12345679_f32, -112.56789, 0.0, 1.0]; 10_000];
        let expected = serde_json::to_string(&frames).unwrap();
        assert_eq!(serialize(&frames, Some(&check)).unwrap(), expected);
        assert!(checks.get() > 2);
        assert_eq!(serialize(&frames, None).unwrap(), expected);
    }

    #[test]
    fn cancellation_before_or_at_completion_never_returns_a_response() {
        let cancelled = || anyhow::bail!("Cursor preview cancelled");
        assert!(serialize(&[1, 2], Some(&cancelled)).is_err());
        let checks = Cell::new(0);
        let at_completion = || {
            checks.set(checks.get() + 1);
            anyhow::ensure!(checks.get() == 1, "Cursor preview cancelled");
            Ok(())
        };
        assert!(serialize(&[1, 2], Some(&at_completion)).is_err());
        assert_eq!(checks.get(), 2);
    }
}

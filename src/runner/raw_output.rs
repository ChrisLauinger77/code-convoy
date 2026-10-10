use crate::process::Stream;

/// Retain stream bytes as text without corrupting split UTF-8 characters.
/// Only an incomplete UTF-8 suffix (at most three bytes per stream) is buffered.
#[derive(Default)]
pub(crate) struct RawOutput {
    pending: [Vec<u8>; 2],
}

impl RawOutput {
    pub fn push(&mut self, stream: Stream, bytes: &[u8]) -> String {
        let pending = &mut self.pending[usize::from(stream == Stream::Stderr)];
        pending.extend_from_slice(bytes);
        let mut text = String::new();
        let mut consumed = 0;
        while consumed < pending.len() {
            match std::str::from_utf8(&pending[consumed..]) {
                Ok(valid) => {
                    text.push_str(valid);
                    consumed = pending.len();
                }
                Err(error) => {
                    let end = consumed + error.valid_up_to();
                    text.push_str(&String::from_utf8_lossy(&pending[consumed..end]));
                    consumed = end;
                    if let Some(count) = error.error_len() {
                        text.push('\u{fffd}');
                        consumed += count;
                    } else {
                        break;
                    }
                }
            }
        }
        pending.drain(..consumed);
        if stream == Stream::Stderr && !text.is_empty() {
            format!("[stderr] {text}")
        } else {
            text
        }
    }
    pub fn finish(&mut self) -> String {
        let mut text = String::new();
        for (index, bytes) in self.pending.iter_mut().enumerate() {
            if !bytes.is_empty() {
                if index == 1 {
                    text.push_str("[stderr] ");
                }
                text.push_str(&String::from_utf8_lossy(bytes));
                bytes.clear();
            }
        }
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_json_and_unicode_survive_byte_sized_chunks() {
        let input = "{\"type\":\"event\",\"text\":\"Grüße 🌍\"}\n";
        let mut decoder = RawOutput::default();
        let mut output = String::new();
        for byte in input.as_bytes() {
            output.push_str(&decoder.push(Stream::Stdout, &[*byte]));
            assert!(decoder.pending[0].len() <= 3);
        }
        output.push_str(&decoder.finish());
        assert_eq!(input, output);
    }
    #[test]
    fn stderr_is_identified_and_invalid_utf8_is_displayed_with_bounded_state() {
        let mut decoder = RawOutput::default();
        assert_eq!(
            decoder.push(Stream::Stderr, b"warning\n"),
            "[stderr] warning\n"
        );
        assert_eq!(decoder.push(Stream::Stdout, b"\xff\xe2"), "�");
        assert_eq!(decoder.finish(), "�");
        assert!(decoder.pending.iter().all(Vec::is_empty));
    }
}

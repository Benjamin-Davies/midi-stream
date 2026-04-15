use wmidi::FromBytesError;

/// Determines how status bytes are encoded or decoded.
pub trait MidiStatusCodec {
    /// Returns true if the given status byte should be encoded.
    fn should_encode_status(&self, status: u8) -> bool;

    /// Update the inner state after a status byte has been encoded.
    fn post_encode(&mut self, status: u8);

    /// Decodes a status byte. Returns the status byte and the number of bytes consumed (0 or 1).
    /// Make sure to only call this method once per status byte as it may update the inner state.
    fn decode_status(&mut self, raw: &[u8]) -> Result<(u8, usize), FromBytesError>;
}

/// Include the status byte with every message (this is the normal way).
#[derive(Debug, Default, Clone)]
pub struct RepeatedStatus;

/// Uses "running status".
///
/// See also: http://midi.teragonaudio.com/tech/midispec/run.htm
#[derive(Debug, Default, Clone)]
pub struct RunningStatus {
    last_status: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StatusClass {
    Voice,
    System,
    RealTime,
}

impl MidiStatusCodec for RepeatedStatus {
    fn should_encode_status(&self, _status: u8) -> bool {
        true
    }

    fn post_encode(&mut self, _status: u8) {
        // Do nothing
    }

    fn decode_status(&mut self, raw: &[u8]) -> Result<(u8, usize), FromBytesError> {
        let status = *raw.first().ok_or(FromBytesError::NoBytes)?;
        Ok((status, 1))
    }
}

impl MidiStatusCodec for RunningStatus {
    fn should_encode_status(&self, status: u8) -> bool {
        if StatusClass::try_from(status) == Ok(StatusClass::Voice) {
            status != self.last_status
        } else {
            true
        }
    }

    fn post_encode(&mut self, status: u8) {
        match StatusClass::try_from(status) {
            Err(_) => {
                // Do nothing
            }
            Ok(StatusClass::Voice) => {
                self.last_status = status;
            }
            Ok(StatusClass::System) => {
                // Cancel the running status
                self.last_status = 0;
            }
            Ok(StatusClass::RealTime) => {
                // Do nothing
            }
        }
    }

    fn decode_status(&mut self, raw: &[u8]) -> Result<(u8, usize), FromBytesError> {
        let status = *raw.iter().next().ok_or(FromBytesError::NoBytes)?;
        match StatusClass::try_from(status) {
            Err(_) => {
                if self.last_status != 0 {
                    Ok((self.last_status, 0))
                } else {
                    Err(FromBytesError::UnexpectedDataByte)
                }
            }
            Ok(StatusClass::Voice) => {
                self.last_status = status;
                Ok((status, 1))
            }
            Ok(StatusClass::System) => {
                // Cancel the running status
                self.last_status = 0;
                Ok((status, 1))
            }
            Ok(StatusClass::RealTime) => Ok((status, 1)),
        }
    }
}

impl TryFrom<u8> for StatusClass {
    type Error = FromBytesError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00..=0x7F => Err(FromBytesError::UnexpectedDataByte),
            0x80..=0xEF => Ok(Self::Voice),
            0xF0..=0xF7 => Ok(Self::System),
            0xF8..=0xFF => Ok(Self::RealTime),
        }
    }
}

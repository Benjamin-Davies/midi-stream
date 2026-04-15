//! Encodes and decodes MIDI streams. Supports running status but not realtime messages in the
//! middle of other messages.

#![no_std]
#![deny(missing_docs)]

#[cfg(feature = "std")]
extern crate std;

mod status;
#[cfg(feature = "tokio")]
pub mod tokio;

pub use wmidi;
use wmidi::{Channel, FromBytesError, MidiMessage, U7, U14};

pub use crate::status::{MidiStatusCodec, RepeatedStatus, RunningStatus};

/// Encodes and decodes streams of MIDI messages.
#[derive(Debug, Default, Clone)]
pub struct MidiCodec<S: MidiStatusCodec = RepeatedStatus> {
    encode_status: S,
    decode_status: S,
}

impl MidiCodec {
    /// Constructs a new MIDI codec without "running status".
    pub fn new() -> Self {
        Self::default()
    }
}

impl MidiCodec<RunningStatus> {
    /// Constructs a new MIDI codec with "running status".
    pub fn with_running_status() -> Self {
        Self::default()
    }
}

impl<S: MidiStatusCodec> MidiCodec<S> {
    /// Returns the number of bytes that would be needed to encode the given message if it were the
    /// next message to be encoded.
    pub fn byte_size(&self, message: &MidiMessage) -> usize {
        let status = status_byte(message);
        let status_len = if self.encode_status.should_encode_status(status) {
            1
        } else {
            0
        };

        let data_len = match message {
            MidiMessage::NoteOff(_, _, _)
            | MidiMessage::NoteOn(_, _, _)
            | MidiMessage::PolyphonicKeyPressure(_, _, _)
            | MidiMessage::ControlChange(_, _, _) => 2,
            MidiMessage::ProgramChange(_, _) | MidiMessage::ChannelPressure(_, _) => 1,
            MidiMessage::PitchBendChange(_, _) => 2,
            MidiMessage::SysEx(b) => b.len() + 1,
            #[cfg(feature = "std")]
            MidiMessage::OwnedSysEx(b) => b.len() + 1,
            MidiMessage::MidiTimeCode(_) => 1,
            MidiMessage::SongPositionPointer(_) => 2,
            MidiMessage::SongSelect(_) => 1,
            MidiMessage::Reserved(_)
            | MidiMessage::TuneRequest
            | MidiMessage::TimingClock
            | MidiMessage::Start
            | MidiMessage::Continue
            | MidiMessage::Stop
            | MidiMessage::ActiveSensing
            | MidiMessage::Reset => 0,
        };

        status_len + data_len
    }

    /// Encodes the given message and returns the number of bytes written. Panics if there isn't
    /// enough space in the buffer.
    pub fn copy_to_slice(&mut self, message: &MidiMessage, buf: &mut [u8]) -> usize {
        let mut cursor = 0;
        let mut put_bytes = |b: &[u8]| {
            buf[cursor..cursor + b.len()].copy_from_slice(b);
            cursor += b.len();
        };

        let status = status_byte(message);
        if self.encode_status.should_encode_status(status) {
            put_bytes(&[status]);
            self.encode_status.post_encode(status);
        }

        match message {
            MidiMessage::NoteOff(_, b, c)
            | MidiMessage::NoteOn(_, b, c)
            | MidiMessage::PolyphonicKeyPressure(_, b, c) => {
                put_bytes(&[u8::from(*b), u8::from(*c)]);
            }
            MidiMessage::ControlChange(_, b, c) => {
                put_bytes(&[u8::from(*b), u8::from(*c)]);
            }
            MidiMessage::ProgramChange(_, b) => {
                put_bytes(&[u8::from(*b)]);
            }
            MidiMessage::ChannelPressure(_, b) => {
                put_bytes(&[u8::from(*b)]);
            }
            MidiMessage::PitchBendChange(_, b) => {
                put_bytes(&split_u14(*b));
            }
            MidiMessage::SysEx(b) => {
                put_bytes(U7::data_to_bytes(b));
                put_bytes(&[0xF7]);
            }
            #[cfg(feature = "std")]
            MidiMessage::OwnedSysEx(b) => {
                put_bytes(U7::data_to_bytes(b));
                put_bytes(&[0xF7]);
            }
            MidiMessage::MidiTimeCode(b) => {
                put_bytes(&[u8::from(*b)]);
            }
            MidiMessage::SongPositionPointer(b) => {
                put_bytes(&split_u14(*b));
            }
            MidiMessage::SongSelect(b) => {
                put_bytes(&[u8::from(*b)]);
            }
            MidiMessage::Reserved(_)
            | MidiMessage::TuneRequest
            | MidiMessage::TimingClock
            | MidiMessage::Start
            | MidiMessage::Continue
            | MidiMessage::Stop
            | MidiMessage::ActiveSensing
            | MidiMessage::Reset => {
                // No data bytes
            }
        }

        cursor
    }

    /// Decodes a message from the given buffer. Returns the message and the number of bytes read.
    /// If the message is a SysEx message then the data will be borrowed.
    pub fn from_bytes<'a>(
        &mut self,
        raw: &'a [u8],
    ) -> Result<(MidiMessage<'a>, usize), FromBytesError> {
        let mut cursor = 0;

        let (status, status_len) = self.decode_status.decode_status(raw)?;
        cursor += status_len;

        let channel = Channel::from_index(status & 0x0F)?;
        let mut get_byte = || {
            let byte = raw[cursor];
            cursor += 1;
            U7::new(byte)
        };
        let message = match status & 0xF0 {
            0x80 => MidiMessage::NoteOff(channel, get_byte()?.into(), get_byte()?),
            0x90 => MidiMessage::NoteOn(channel, get_byte()?.into(), get_byte()?),
            0xA0 => MidiMessage::PolyphonicKeyPressure(channel, get_byte()?.into(), get_byte()?),
            0xB0 => MidiMessage::ControlChange(channel, get_byte()?.into(), get_byte()?),
            0xC0 => MidiMessage::ProgramChange(channel, get_byte()?),
            0xD0 => MidiMessage::ChannelPressure(channel, get_byte()?),
            0xE0 => MidiMessage::PitchBendChange(channel, combine_u14(get_byte()?, get_byte()?)),
            0xF0 => match status {
                0xF0 => {
                    let data_len = raw[cursor..]
                        .iter()
                        .position(|b| *b == 0xF7)
                        .ok_or(FromBytesError::NoSysExEndByte)?;
                    let data = U7::try_from_bytes(&raw[cursor..cursor + data_len])?;
                    cursor += data_len + 1;
                    MidiMessage::SysEx(data)
                }
                0xF1 => MidiMessage::MidiTimeCode(get_byte()?),
                0xF2 => MidiMessage::SongPositionPointer(combine_u14(get_byte()?, get_byte()?)),
                0xF3 => MidiMessage::SongSelect(get_byte()?),
                0xF4 | 0xF5 => MidiMessage::Reserved(status),
                0xF6 => MidiMessage::TuneRequest,
                0xF7 => return Err(FromBytesError::UnexpectedEndSysExByte),
                0xF8 => MidiMessage::TimingClock,
                0xF9 => MidiMessage::Reserved(status),
                0xFA => MidiMessage::Start,
                0xFB => MidiMessage::Continue,
                0xFC => MidiMessage::Stop,
                0xFD => MidiMessage::Reserved(status),
                0xFE => MidiMessage::ActiveSensing,
                0xFF => MidiMessage::Reset,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };

        Ok((message, cursor))
    }
}

fn status_byte(message: &MidiMessage) -> u8 {
    match message {
        MidiMessage::NoteOff(a, _, _) => 0x80 | a.index(),
        MidiMessage::NoteOn(a, _, _) => 0x90 | a.index(),
        MidiMessage::PolyphonicKeyPressure(a, _, _) => 0xA0 | a.index(),
        MidiMessage::ControlChange(a, _, _) => 0xB0 | a.index(),
        MidiMessage::ProgramChange(a, _) => 0xC0 | a.index(),
        MidiMessage::ChannelPressure(a, _) => 0xD0 | a.index(),
        MidiMessage::PitchBendChange(a, _) => 0xE0 | a.index(),
        MidiMessage::SysEx(_) => 0xF0,
        #[cfg(feature = "std")]
        MidiMessage::OwnedSysEx(_) => 0xF0,
        MidiMessage::MidiTimeCode(_) => 0xF1,
        MidiMessage::SongPositionPointer(_) => 0xF2,
        MidiMessage::SongSelect(_) => 0xF3,
        MidiMessage::Reserved(a) => *a,
        MidiMessage::TuneRequest => 0xF6,
        MidiMessage::TimingClock => 0xF8,
        MidiMessage::Start => 0xFA,
        MidiMessage::Continue => 0xFB,
        MidiMessage::Stop => 0xFC,
        MidiMessage::ActiveSensing => 0xFE,
        MidiMessage::Reset => 0xFF,
    }
}

fn combine_u14(lower: U7, higher: U7) -> U14 {
    let raw = u16::from(u8::from(lower)) + u16::from(u8::from(higher)) << 7;
    U14::try_from(raw).unwrap()
}

fn split_u14(data: U14) -> [u8; 2] {
    let raw = u16::from(data);
    [(raw & 0x007F) as u8, (raw >> 7) as u8]
}

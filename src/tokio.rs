//! Uses [`tokio_util::codec`] to encode and decode MIDI streams.

use std::io;

use tokio_util::{
    bytes::{Buf, BufMut, BytesMut},
    codec::{Decoder, Encoder},
};
use wmidi::{FromBytesError, MidiMessage};

use crate::{MidiCodec, MidiStatusCodec};

/// An error that may occur while encoding a message.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    /// A parser error.
    #[error(transparent)]
    Parser(#[from] FromBytesError),
    /// An IO error.
    #[error(transparent)]
    IO(#[from] io::Error),
}

impl<'a, S: MidiStatusCodec> Encoder<MidiMessage<'a>> for MidiCodec<S> {
    type Error = io::Error;

    fn encode(&mut self, message: MidiMessage, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let start = dst.len();
        let len = self.byte_size(&message);

        dst.put_bytes(0, len);
        self.copy_to_slice(&message, &mut dst[start..]);

        Ok(())
    }
}

impl<S: MidiStatusCodec> Decoder for MidiCodec<S> {
    type Item = MidiMessage<'static>;
    type Error = DecodeError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        match self.from_bytes(&src) {
            Ok((message, len)) => {
                let message = message.to_owned();
                src.advance(len);
                Ok(Some(message))
            }
            Err(FromBytesError::NoBytes)
            | Err(FromBytesError::NotEnoughBytes)
            | Err(FromBytesError::NoSysExEndByte) => Ok(None),
            Err(err) => Err(err.into()),
        }
    }
}

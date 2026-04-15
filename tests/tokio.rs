#![cfg(feature = "tokio")]

use midi_stream::MidiCodec;
use tokio_util::{
    bytes::{BufMut, BytesMut},
    codec::{Decoder, Encoder},
};
use wmidi::{Channel, MidiMessage, Note, U7};

fn u7(b: u8) -> U7 {
    b.try_into().unwrap()
}

fn u7_hex(hex: &str) -> Vec<U7> {
    let bytes = hex::decode(hex).unwrap();
    U7::try_from_bytes(&bytes).unwrap().to_vec()
}

fn make_messages() -> Vec<MidiMessage<'static>> {
    vec![
        MidiMessage::NoteOn(Channel::Ch1, Note::C5, u7(0x7F)),
        MidiMessage::OwnedSysEx(u7_hex("01234567")),
        MidiMessage::NoteOn(Channel::Ch1, Note::G5, u7(0x7F)),
        MidiMessage::NoteOn(Channel::Ch1, Note::B5, u7(0x7F)),
        MidiMessage::NoteOff(Channel::Ch1, Note::B5, u7(0x00)),
        MidiMessage::NoteOn(Channel::Ch2, Note::C6, u7(0x7F)),
        MidiMessage::Reset,
        MidiMessage::NoteOn(Channel::Ch2, Note::E6, u7(0x7F)),
    ]
}

const RAW_BYTES_REPEATED: &str = "90487FF001234567F7904F7F90537F80530091547FFF91587F";
const RAW_BYTES_RUNNING_STATUS: &str = "90487FF001234567F7904F7F537F80530091547FFF587F";

#[test]
fn test_encode_repeated() {
    let mut codec = MidiCodec::new();
    let mut buf = BytesMut::new();

    for message in make_messages() {
        codec.encode(message, &mut buf).unwrap();
    }

    assert_eq!(hex::encode_upper(buf), RAW_BYTES_REPEATED);
}

#[test]
fn test_encode_running_status() {
    let mut codec = MidiCodec::with_running_status();
    let mut buf = BytesMut::new();

    for message in make_messages() {
        codec.encode(message, &mut buf).unwrap();
    }

    assert_eq!(hex::encode_upper(buf), RAW_BYTES_RUNNING_STATUS);
}

#[test]
fn test_encode_wmidi() {
    let mut buf = Vec::new();

    for msg in make_messages() {
        let mut msg_buf = vec![0; msg.bytes_size()];
        msg.copy_to_slice(&mut msg_buf).unwrap();
        buf.extend(msg_buf);
    }

    assert_eq!(hex::encode_upper(buf), RAW_BYTES_REPEATED);
}

#[test]
fn test_decode_repeated() {
    let mut codec = MidiCodec::new();
    let mut buf = BytesMut::from(hex::decode(RAW_BYTES_REPEATED).unwrap().as_slice());
    let mut messages = Vec::new();

    while let Some(msg) = codec.decode(&mut buf).unwrap() {
        messages.push(msg);
    }

    assert_eq!(messages, make_messages());
}

#[test]
fn test_decode_running_status() {
    let mut codec = MidiCodec::with_running_status();
    let mut buf = BytesMut::from(hex::decode(RAW_BYTES_RUNNING_STATUS).unwrap().as_slice());
    let mut messages = Vec::new();

    while let Some(msg) = codec.decode(&mut buf).unwrap() {
        messages.push(msg);
    }

    assert_eq!(messages, make_messages());
}

#[test]
fn test_decode_repeated_trickle() {
    let mut codec = MidiCodec::new();
    let mut buf = BytesMut::new();
    let mut messages = Vec::new();

    for b in hex::decode(RAW_BYTES_REPEATED).unwrap() {
        buf.put_u8(b);
        while let Some(msg) = codec.decode(&mut buf).unwrap() {
            messages.push(msg);
        }
    }

    assert_eq!(messages, make_messages());
}

#[test]
fn test_decode_running_status_trickle() {
    let mut codec = MidiCodec::with_running_status();
    let mut buf = BytesMut::new();
    let mut messages = Vec::new();

    for b in hex::decode(RAW_BYTES_RUNNING_STATUS).unwrap() {
        buf.put_u8(b);
        while let Some(msg) = codec.decode(&mut buf).unwrap() {
            messages.push(msg);
        }
    }

    assert_eq!(messages, make_messages());
}

//! I/O for the neuron protocol over TCP (blocking, threads).
//!
//! Each connection runs in its own thread. Messages are framed
//! with a 4-byte length prefix for zero-copy reading.

use std::io::{Read, Write};
use std::net::TcpStream;

use crate::header::{parse_frame, HeaderError, MessageHeader};
use crate::HEADER_SIZE;

/// Read a complete framed message from a TCP stream.
/// Returns the raw frame bytes for zero-copy parsing.
pub fn read_frame(stream: &mut TcpStream, buf: &mut Vec<u8>) -> Result<(), IoError> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).map_err(|e| io_error(e.kind()))?;
    let frame_len = u32::from_le_bytes(len_buf) as usize;

    if frame_len > crate::MAX_BODY_SIZE as usize + HEADER_SIZE {
        return Err(IoError::FrameTooLarge(frame_len));
    }

    buf.resize(frame_len, 0);
    stream.read_exact(buf).map_err(|e| io_error(e.kind()))?;
    Ok(())
}

/// Write a complete framed message to a TCP stream.
pub fn write_frame(stream: &mut TcpStream, msg: &[u8]) -> Result<(), IoError> {
    let frame_len = msg.len() as u32;
    stream.write_all(&frame_len.to_le_bytes()).map_err(|e| io_error(e.kind()))?;
    stream.write_all(msg).map_err(|e| io_error(e.kind()))?;
    Ok(())
}

/// Read and parse a message in one call.
/// Returns (header_ref, body_ref) pointing into `buf`.
pub fn read_message<'a>(
    stream: &mut TcpStream,
    buf: &'a mut Vec<u8>,
) -> Result<(&'a MessageHeader, &'a [u8]), IoError> {
    read_frame(stream, buf)?;
    let (header, body) = parse_frame(buf).map_err(IoError::BadHeader)?;
    Ok((header, body))
}

fn io_error(kind: std::io::ErrorKind) -> IoError {
    if kind == std::io::ErrorKind::UnexpectedEof {
        IoError::ConnectionClosed
    } else {
        IoError::Read(std::io::Error::new(kind, "i/o error"))
    }
}

/// Errors
#[derive(Debug)]
pub enum IoError {
    Read(std::io::Error),
    Write(std::io::Error),
    BadHeader(HeaderError),
    FrameTooLarge(usize),
    ConnectionClosed,
}

impl core::fmt::Display for IoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            IoError::Read(e) => write!(f, "read: {}", e),
            IoError::Write(e) => write!(f, "write: {}", e),
            IoError::BadHeader(e) => write!(f, "header: {}", e),
            IoError::FrameTooLarge(n) => write!(f, "frame too large: {}B", n),
            IoError::ConnectionClosed => write!(f, "connection closed"),
        }
    }
}

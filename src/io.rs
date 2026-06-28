//! I/O for the neuron protocol over TCP (blocking, threads).
//!
//! Each connection runs in its own thread. Messages are framed
//! with a 4-byte length prefix for zero-copy reading.

#![allow(missing_docs)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_io_error_display() {
        let e = IoError::ConnectionClosed;
        assert_eq!(format!("{}", e), "connection closed");
        let e = IoError::FrameTooLarge(999999);
        assert!(format!("{}", e).contains("999999"));
        let header_err = crate::header::HeaderError::ShortBuffer(4);
        let e = IoError::BadHeader(header_err);
        assert!(format!("{}", e).contains("header:"));
    }

    #[test]
    fn test_read_write_frame_loopback() {
        use std::net::{TcpListener, TcpStream};
        use std::thread;
        use std::time::Duration;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let mut buf = Vec::new();
            read_frame(&mut stream, &mut buf)
        });

        let mut client = TcpStream::connect(addr).unwrap();
        // Send raw header+body (write_frame adds its own 4-byte len)
        let h = crate::header::MessageHeader::new(0, 0, 0);
        write_frame(&mut client, &h.to_bytes()).unwrap();
        drop(client);

        let result = handle.join().unwrap();
        assert!(result.is_ok(), "read_frame loopback failed: {:?}", result);
    }

    #[test]
    fn test_read_message_loopback() {
        use std::net::{TcpListener, TcpStream};
        use std::thread;
        use std::time::Duration;

        let body = vec![0x01, 0x02, 0x03, 0x04];
        let h = crate::header::MessageHeader::new(5, body.len() as u32, 0);
        let mut msg = Vec::with_capacity(crate::HEADER_SIZE + body.len());
        msg.extend_from_slice(&h.to_bytes());
        msg.extend_from_slice(&body);

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
            let mut buf = Vec::new();
            read_frame(&mut stream, &mut buf).ok()?;
            let (h, b) = crate::header::parse_frame(&buf).ok()?;
            Some((h.msg_type, b.to_vec()))
        });

        let mut client = TcpStream::connect(addr).unwrap();
        write_frame(&mut client, &msg).unwrap();
        drop(client);

        let result = handle.join().unwrap();
        assert!(result.is_some(), "read_message loopback failed");
        let (msg_type, parsed_body) = result.unwrap();
        assert_eq!(msg_type, 5);
        assert_eq!(parsed_body, body);
    }

    #[test]
    fn test_frame_too_large_error() {
        let e = IoError::FrameTooLarge(1_000_000_100);
        assert!(format!("{}", e).contains("frame too large"));
    }
}

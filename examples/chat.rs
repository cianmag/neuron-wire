//! Chat Example — Interactive P2P Messaging
//!
//! A simple chat application built on neuron-wire's transport layer.
//! Start one or more nodes and send messages between them.
//!
//! Usage:
//!     # Start a node (Alice on port 9000)
//!     cargo run --example chat -- --name alice --port 9000
//!
//!     # Connect another node (Bob on port 9001, connecting to Alice)
//!     cargo run --example chat -- --name bob --port 9001 --connect 127.0.0.1:9000
//!
//! Once connected, type messages and press Enter to broadcast.

use std::io::{self, BufRead, Write};
use std::net::UdpSocket;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

fn parse_args() -> (String, u16, Option<String>) {
    let args: Vec<String> = std::env::args().collect();
    let mut name = "node".to_string();
    let mut port = 9000u16;
    let mut connect: Option<String> = None;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--name" => {
                i += 1;
                name = args.get(i).cloned().unwrap_or(name);
            }
            "--port" => {
                i += 1;
                port = args
                    .get(i)
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(port);
            }
            "--connect" => {
                i += 1;
                connect = args.get(i).cloned();
            }
            _ => {}
        }
        i += 1;
    }
    (name, port, connect)
}

fn main() {
    let (name, port, connect_to) = parse_args();

    println!("═══ NEURON-WIRE CHAT ═══");
    println!("  Name: {name}");
    println!("  Port: {port}");
    if let Some(ref addr) = connect_to {
        println!("  Connecting to: {addr}");
    }
    println!();
    println!("Type a message and press Enter to broadcast.");
    println!("Press Ctrl+C to exit.");
    println!();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    // Set up a simple UDP socket for demonstration
    let socket = UdpSocket::bind(format!("0.0.0.0:{port}")).expect("Failed to bind UDP socket");
    socket
        .set_read_timeout(Some(Duration::from_millis(200)))
        .ok();
    let socket = Arc::new(socket);

    // If we have a peer to connect to, send a handshake
    if let Some(ref peer_addr) = connect_to {
        let msg = format!("HELO {}:{port}", name);
        socket
            .send_to(msg.as_bytes(), peer_addr)
            .expect("Failed to send handshake");
        println!("→ Sent handshake to {peer_addr}");
    }

    // Receiver thread
    let sock_rx = socket.clone();
    let _name_rx = name.clone();
    thread::spawn(move || {
        let mut buf = [0u8; 4096];
        while r.load(Ordering::Relaxed) {
            match sock_rx.recv_from(&mut buf) {
                Ok((n, src)) => {
                    let msg = String::from_utf8_lossy(&buf[..n]);
                    println!("  [{src}] {msg}");
                }
                Err(_) => {} // timeout, loop
            }
        }
    });

    // Main loop — read from stdin and send
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        match line {
            Ok(text) => {
                let msg = format!("[{name}] {text}");
                // Send to broadcast address or specific peer
                if let Some(ref peer) = connect_to {
                    socket.send_to(msg.as_bytes(), peer).ok();
                } else {
                    // Listen mode — just display locally
                    println!("  [local] {text}");
                }
                stdout.flush().ok();
            }
            Err(_) => break,
        }
    }

    running.store(false, Ordering::Relaxed);
    println!("\nChat ended.");
}

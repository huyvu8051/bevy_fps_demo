use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use renet2_netcode::{ClientSocket, NetcodeTransportError, ServerSocket};

// Reads SIM_LATENCY_MS (milliseconds, unset/invalid = 0 = disabled) — set
// it on the client, the server, or both to simulate a real network's
// transit delay for local/LAN dev testing. Delaying sends on just one side
// of a hop models that hop's one-way latency without double-counting, so
// setting it on both ends gives a full simulated round trip (e.g. 50 on
// both sides ≈ 100ms RTT), while setting it on only one side gives a
// one-way-only version if that's what you're after.
pub fn sim_latency_from_env() -> Duration {
    let ms: u64 = std::env::var("SIM_LATENCY_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    Duration::from_millis(ms)
}

// Wraps a real ServerSocket/ClientSocket, holding outgoing packets for
// `delay` before actually handing them to the inner socket. Incoming
// packets (try_recv) pass straight through — see sim_latency_from_env's
// doc comment for why only delaying sends is enough.
#[derive(Debug)]
pub struct LaggedSocket<S> {
    inner: S,
    delay: Duration,
    pending: VecDeque<(Instant, SocketAddr, Vec<u8>)>,
}

impl<S> LaggedSocket<S> {
    pub fn new(inner: S, delay: Duration) -> Self {
        Self {
            inner,
            delay,
            pending: VecDeque::new(),
        }
    }
}

impl<S: ServerSocket> ServerSocket for LaggedSocket<S> {
    fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }
    fn is_reliable(&self) -> bool {
        self.inner.is_reliable()
    }
    fn addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.addr()
    }
    fn is_closed(&mut self) -> bool {
        self.inner.is_closed()
    }
    fn close(&mut self) {
        self.inner.close();
    }
    fn connection_denied(&mut self, addr: SocketAddr) {
        self.inner.connection_denied(addr);
    }
    fn connection_accepted(&mut self, client_id: u64, addr: SocketAddr) {
        self.inner.connection_accepted(client_id, addr);
    }
    fn disconnect(&mut self, addr: SocketAddr) {
        self.inner.disconnect(addr);
    }
    fn preupdate(&mut self) {
        self.inner.preupdate();
    }
    fn try_recv(&mut self, buffer: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        self.inner.try_recv(buffer)
    }
    fn postupdate(&mut self) {
        if !self.delay.is_zero() {
            let now = Instant::now();
            while let Some((due, ..)) = self.pending.front() {
                if *due > now {
                    break;
                }
                let (_, addr, packet) = self.pending.pop_front().expect("front just checked Some");
                let _ = self.inner.send(addr, &packet);
            }
        }
        self.inner.postupdate();
    }
    fn send(&mut self, addr: SocketAddr, packet: &[u8]) -> Result<(), NetcodeTransportError> {
        if self.delay.is_zero() {
            return self.inner.send(addr, packet);
        }
        self.pending.push_back((Instant::now() + self.delay, addr, packet.to_vec()));
        Ok(())
    }
}

impl<S: ClientSocket> ClientSocket for LaggedSocket<S> {
    fn is_encrypted(&self) -> bool {
        self.inner.is_encrypted()
    }
    fn is_reliable(&self) -> bool {
        self.inner.is_reliable()
    }
    fn addr(&self) -> std::io::Result<SocketAddr> {
        self.inner.addr()
    }
    fn is_closed(&mut self) -> bool {
        self.inner.is_closed()
    }
    fn close(&mut self) {
        self.inner.close();
    }
    fn preupdate(&mut self) {
        self.inner.preupdate();
    }
    fn try_recv(&mut self, buffer: &mut [u8]) -> std::io::Result<(usize, SocketAddr)> {
        self.inner.try_recv(buffer)
    }
    fn postupdate(&mut self) {
        if !self.delay.is_zero() {
            let now = Instant::now();
            while let Some((due, ..)) = self.pending.front() {
                if *due > now {
                    break;
                }
                let (_, addr, packet) = self.pending.pop_front().expect("front just checked Some");
                let _ = self.inner.send(addr, &packet);
            }
        }
        self.inner.postupdate();
    }
    fn send(&mut self, addr: SocketAddr, packet: &[u8]) -> Result<(), NetcodeTransportError> {
        if self.delay.is_zero() {
            return self.inner.send(addr, packet);
        }
        self.pending.push_back((Instant::now() + self.delay, addr, packet.to_vec()));
        Ok(())
    }
}

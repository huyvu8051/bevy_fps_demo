use std::collections::VecDeque;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use renet2_netcode::{ClientSocket, NetcodeTransportError, ServerSocket};

// Base delay + jitter (whole ms) + packet-loss fraction — see
// sim_latency_from_env.
#[derive(Debug, Clone, Copy)]
pub struct SimLatencyConfig {
    pub base: Duration,
    pub jitter: Duration,
    // 0.0..=1.0 — chance each outgoing packet is silently dropped.
    pub loss: f32,
}

// Reads SIM_LATENCY_MS (base delay) and SIM_LATENCY_JITTER_MS (extra random
// 0..=jitter added per packet, unset/invalid = 0 = disabled) — set on the
// client, the server, or both to simulate a real network's transit delay
// for local/LAN dev testing. Delaying sends on just one side of a hop
// models that hop's one-way latency without double-counting, so setting it
// on both ends gives a full simulated round trip (e.g. 50 on both sides ≈
// 100ms RTT), while setting it on only one side gives a one-way-only
// version if that's what you're after. Jitter is randomized independently
// per packet (see LaggedSocket's own xorshift), so a real connection's
// packet-to-packet variance is modeled too, not just a flat extra delay.
// SIM_PACKET_LOSS_PCT (0..=100, unset/invalid = 0) drops that % of
// outgoing packets — set on either/both ends the same way as the delay.
pub fn sim_latency_from_env() -> SimLatencyConfig {
    let read_ms = |var: &str| -> u64 { std::env::var(var).ok().and_then(|s| s.parse().ok()).unwrap_or(0) };
    let loss_pct: f32 = std::env::var("SIM_PACKET_LOSS_PCT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0.0);
    SimLatencyConfig {
        base: Duration::from_millis(read_ms("SIM_LATENCY_MS")),
        jitter: Duration::from_millis(read_ms("SIM_LATENCY_JITTER_MS")),
        loss: (loss_pct / 100.0).clamp(0.0, 1.0),
    }
}

// Wraps a real ServerSocket/ClientSocket, holding outgoing packets for
// `base + random(0..=jitter)` before actually handing them to the inner
// socket. Incoming packets (try_recv) pass straight through — see
// sim_latency_from_env's doc comment for why only delaying sends is enough.
#[derive(Debug)]
pub struct LaggedSocket<S> {
    inner: S,
    config: SimLatencyConfig,
    // Small xorshift64 PRNG for per-packet jitter — not cryptographic, just
    // needs to look random enough for dev testing, and this avoids pulling
    // in the `rand` crate for one call site.
    rng_state: u64,
    pending: VecDeque<(Instant, SocketAddr, Vec<u8>)>,
}

impl<S> LaggedSocket<S> {
    pub fn new(inner: S, config: SimLatencyConfig) -> Self {
        let seed = Instant::now().elapsed().as_nanos() as u64 ^ 0x9E3779B97F4A7C15;
        Self {
            inner,
            config,
            rng_state: if seed == 0 { 1 } else { seed },
            pending: VecDeque::new(),
        }
    }

    fn next_delay(&mut self) -> Duration {
        if self.config.jitter.is_zero() {
            return self.config.base;
        }
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        let frac = (self.rng_state >> 11) as f64 / (1u64 << 53) as f64;
        self.config.base + self.config.jitter.mul_f64(frac)
    }

    fn has_delay(&self) -> bool {
        !self.config.base.is_zero() || !self.config.jitter.is_zero()
    }

    // True with probability `config.loss`, consuming from the same xorshift
    // stream next_delay uses.
    fn should_drop(&mut self) -> bool {
        if self.config.loss <= 0.0 {
            return false;
        }
        self.rng_state ^= self.rng_state << 13;
        self.rng_state ^= self.rng_state >> 7;
        self.rng_state ^= self.rng_state << 17;
        let frac = (self.rng_state >> 11) as f64 / (1u64 << 53) as f64;
        (frac as f32) < self.config.loss
    }
}

impl<S: ServerSocket> LaggedSocket<S> {
    // Named distinctly from ClientSocket's flush_due_client below —
    // Rust can't prove ServerSocket/ClientSocket are mutually exclusive
    // bounds, so two same-named inherent methods here would conflict even
    // though in practice only one impl ever applies to a given S. Scans the
    // whole queue rather than assuming `pending` stays sorted by due time —
    // per-packet jitter means a later-queued packet can end up due before
    // an earlier one, so a front-only check (fine for a flat delay) would
    // stall everything behind whichever packet drew the longest jitter.
    fn flush_due_server(&mut self) {
        let now = Instant::now();
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                let (_, addr, packet) = self.pending.remove(i).expect("index just checked in-bounds");
                let _ = self.inner.send(addr, &packet);
            } else {
                i += 1;
            }
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
        self.flush_due_server();
        self.inner.postupdate();
    }
    fn send(&mut self, addr: SocketAddr, packet: &[u8]) -> Result<(), NetcodeTransportError> {
        if self.should_drop() {
            return Ok(()); // silently lost — reliable channels retransmit
        }
        if !self.has_delay() {
            return self.inner.send(addr, packet);
        }
        let due = Instant::now() + self.next_delay();
        self.pending.push_back((due, addr, packet.to_vec()));
        Ok(())
    }
}

impl<S: ClientSocket> LaggedSocket<S> {
    // See ServerSocket's own flush_due_server for why this scans instead of
    // assuming sorted-by-due-time (and why it needs its own name).
    fn flush_due_client(&mut self) {
        let now = Instant::now();
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                let (_, addr, packet) = self.pending.remove(i).expect("index just checked in-bounds");
                let _ = self.inner.send(addr, &packet);
            } else {
                i += 1;
            }
        }
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
        self.flush_due_client();
        self.inner.postupdate();
    }
    fn send(&mut self, addr: SocketAddr, packet: &[u8]) -> Result<(), NetcodeTransportError> {
        if self.should_drop() {
            return Ok(()); // silently lost — reliable channels retransmit
        }
        if !self.has_delay() {
            return self.inner.send(addr, packet);
        }
        let due = Instant::now() + self.next_delay();
        self.pending.push_back((due, addr, packet.to_vec()));
        Ok(())
    }
}

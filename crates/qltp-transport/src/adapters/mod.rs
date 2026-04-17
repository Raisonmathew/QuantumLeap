//! Adapters layer - Concrete transport implementations

pub mod tcp;

#[cfg(feature = "io_uring")]
pub mod io_uring;

#[cfg(feature = "quic")]
pub mod quic;

// DPDK adapter (future implementation)
// #[cfg(feature = "dpdk")]
// pub mod dpdk;

pub use tcp::TcpBackend;

#[cfg(feature = "io_uring")]
pub use self::io_uring::IoUringBackend;

#[cfg(feature = "quic")]
pub use quic::QuicBackend;

// #[cfg(feature = "dpdk")]
// pub use dpdk::DpdkBackend;

// Made with Bob

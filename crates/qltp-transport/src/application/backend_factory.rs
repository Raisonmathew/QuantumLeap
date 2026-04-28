//! Backend Factory - Constructs concrete transport backends from a `TransportType`.
//!
//! This is the missing link between [`BackendSelector`] (which decides *which*
//! backend should be used) and [`TransportManager`] (which needs a ready-to-use
//! backend instance). Selection logic stays platform/criteria aware; this
//! module is the single place that turns a chosen [`TransportType`] into a
//! `Box<dyn TransportBackend>`.
//!
//! Cross-platform notes:
//! - `io_uring` is only available on Linux **and** when the `io_uring` cargo
//!   feature is enabled. On other targets the factory returns a clear
//!   configuration error rather than silently downgrading.
//! - `quic` requires the `quic` cargo feature.
//! - `dpdk` is intentionally unsupported here because no real adapter exists.

use crate::adapters::TcpBackend;
use crate::domain::TransportType;
use crate::error::{Error, Result};
use crate::ports::TransportBackend;

/// Build a concrete, *uninitialized* backend for the given transport type.
///
/// The caller is expected to call [`TransportBackend::initialize`] (typically
/// via [`crate::application::TransportManager::initialize`]) before using the
/// returned backend.
pub fn build_backend(transport_type: TransportType) -> Result<Box<dyn TransportBackend>> {
    match transport_type {
        TransportType::Tcp => Ok(Box::new(TcpBackend::new())),

        #[cfg(feature = "quic")]
        TransportType::Quic => Ok(Box::new(crate::adapters::QuicBackend::with_defaults())),

        #[cfg(not(feature = "quic"))]
        TransportType::Quic => Err(Error::Configuration(
            "QUIC backend requested but the `quic` cargo feature is disabled".to_string(),
        )),

        #[cfg(all(feature = "io_uring", target_os = "linux"))]
        TransportType::IoUring => {
            let backend = crate::adapters::IoUringBackend::new()
                .map_err(|e| Error::Configuration(format!("Failed to create io_uring backend: {}", e)))?;
            Ok(Box::new(backend))
        }

        #[cfg(not(all(feature = "io_uring", target_os = "linux")))]
        TransportType::IoUring => Err(Error::Configuration(
            "io_uring backend is only available on Linux with the `io_uring` cargo feature enabled"
                .to_string(),
        )),

        TransportType::Dpdk => Err(Error::Configuration(
            "DPDK backend is not implemented in this build (requires specialized hardware and is \
             not suitable for cloud deployments)"
                .to_string(),
        )),
    }
}

/// Returns the list of transport types this build *can* construct.
///
/// This is stricter than [`crate::application::BackendSelector::list_available_backends`]
/// which is platform-aware but does not consider compile-time cargo features.
pub fn buildable_transports() -> Vec<TransportType> {
    let mut out = vec![TransportType::Tcp];

    #[cfg(feature = "quic")]
    out.push(TransportType::Quic);

    #[cfg(all(feature = "io_uring", target_os = "linux"))]
    out.push(TransportType::IoUring);

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_is_always_buildable() {
        assert!(buildable_transports().contains(&TransportType::Tcp));
        assert!(build_backend(TransportType::Tcp).is_ok());
    }

    #[test]
    fn dpdk_is_never_buildable() {
        assert!(!buildable_transports().contains(&TransportType::Dpdk));
        assert!(build_backend(TransportType::Dpdk).is_err());
    }

    #[test]
    fn io_uring_outside_linux_is_an_error() {
        if !cfg!(all(feature = "io_uring", target_os = "linux")) {
            assert!(build_backend(TransportType::IoUring).is_err());
        }
    }
}

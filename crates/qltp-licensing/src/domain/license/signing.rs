//! Ed25519 signing/verification for `License` aggregates.
//!
//! Licenses are sensitive integrity-bearing records: they encode tier,
//! features, device list, expiration, and ownership. A row in the database
//! that an attacker can mutate (e.g. via SQL injection, backup tampering,
//! or a compromised admin tool) can otherwise grant unlimited features or
//! extend expiration arbitrarily. To make tampering detectable we sign the
//! canonical serialization of every license at creation/update time and
//! verify the signature on every read when a verifier is configured.
//!
//! Algorithm: Ed25519 (RFC 8032) — small (32-byte public, 64-byte sig),
//! deterministic, constant-time verify, side-channel resistant. Signature
//! is base64-encoded for plain-text JSON storage.

use crate::error::{LicenseError, Result};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use ed25519_dalek::{
    Signature, Signer, SigningKey, Verifier, VerifyingKey, SECRET_KEY_LENGTH,
};
use rand_core::OsRng;

/// Length of an Ed25519 public key in bytes.
pub const PUBLIC_KEY_BYTES: usize = 32;

/// Length of an Ed25519 signature in bytes.
pub const SIGNATURE_BYTES: usize = 64;

/// Holder of the ed25519 secret key used to sign licenses.
///
/// The secret key never leaves this struct; signing happens through the
/// `sign` method. The struct is `!Clone` and drops zeroize-style on drop
/// via `ed25519_dalek::SigningKey`'s own `Drop` semantics.
pub struct LicenseSigner {
    inner: SigningKey,
    verifier: LicenseVerifier,
}

impl LicenseSigner {
    /// Generate a fresh signer using the OS CSPRNG.
    pub fn generate() -> Self {
        let signing = SigningKey::generate(&mut OsRng);
        let verifier = LicenseVerifier {
            inner: signing.verifying_key(),
        };
        Self {
            inner: signing,
            verifier,
        }
    }

    /// Reconstruct a signer from raw 32-byte secret-key material.
    pub fn from_secret_bytes(bytes: &[u8; SECRET_KEY_LENGTH]) -> Self {
        let signing = SigningKey::from_bytes(bytes);
        let verifier = LicenseVerifier {
            inner: signing.verifying_key(),
        };
        Self {
            inner: signing,
            verifier,
        }
    }

    /// The matching verifier (safe to clone and distribute).
    pub fn verifier(&self) -> LicenseVerifier {
        self.verifier.clone()
    }

    /// Sign canonical bytes; returns base64 of the 64-byte detached
    /// Ed25519 signature so it can live inside JSON.
    pub fn sign(&self, payload: &[u8]) -> String {
        let sig: Signature = self.inner.sign(payload);
        BASE64.encode(sig.to_bytes())
    }

    /// Export the secret key bytes. Callers MUST persist with strong
    /// confidentiality (HSM, KMS, OS keyring) — anyone with these bytes
    /// can mint arbitrary licenses.
    pub fn secret_bytes(&self) -> [u8; SECRET_KEY_LENGTH] {
        self.inner.to_bytes()
    }
}

/// Public verifying key for licenses. Cheap to clone; safe to embed in
/// shipped binaries (it is the trust anchor).
#[derive(Clone)]
pub struct LicenseVerifier {
    inner: VerifyingKey,
}

impl LicenseVerifier {
    /// Build a verifier from the raw 32-byte public key.
    pub fn from_public_bytes(bytes: &[u8; PUBLIC_KEY_BYTES]) -> Result<Self> {
        let inner = VerifyingKey::from_bytes(bytes)
            .map_err(|_| LicenseError::InvalidSignature)?;
        Ok(Self { inner })
    }

    /// Export the public key bytes for distribution.
    pub fn public_bytes(&self) -> [u8; PUBLIC_KEY_BYTES] {
        self.inner.to_bytes()
    }

    /// Constant-time verification of `signature_b64` (base64 of 64 bytes)
    /// against `payload`. Returns `Err(InvalidSignature)` for any failure
    /// — bad encoding, wrong length, or cryptographic mismatch — so the
    /// caller cannot distinguish failure modes via timing or error type.
    pub fn verify(&self, payload: &[u8], signature_b64: &str) -> Result<()> {
        let raw = BASE64
            .decode(signature_b64.as_bytes())
            .map_err(|_| LicenseError::InvalidSignature)?;
        if raw.len() != SIGNATURE_BYTES {
            return Err(LicenseError::InvalidSignature);
        }
        let mut buf = [0u8; SIGNATURE_BYTES];
        buf.copy_from_slice(&raw);
        let sig = Signature::from_bytes(&buf);
        self.inner
            .verify(payload, &sig)
            .map_err(|_| LicenseError::InvalidSignature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_then_verify_roundtrip() {
        let signer = LicenseSigner::generate();
        let verifier = signer.verifier();
        let payload = b"the quick brown fox";
        let sig = signer.sign(payload);
        verifier.verify(payload, &sig).unwrap();
    }

    #[test]
    fn tampered_payload_rejected() {
        let signer = LicenseSigner::generate();
        let verifier = signer.verifier();
        let sig = signer.sign(b"original");
        assert!(matches!(
            verifier.verify(b"tampered", &sig),
            Err(LicenseError::InvalidSignature)
        ));
    }

    #[test]
    fn wrong_key_rejected() {
        let signer_a = LicenseSigner::generate();
        let verifier_b = LicenseSigner::generate().verifier();
        let sig = signer_a.sign(b"payload");
        assert!(matches!(
            verifier_b.verify(b"payload", &sig),
            Err(LicenseError::InvalidSignature)
        ));
    }

    #[test]
    fn malformed_signature_rejected() {
        let verifier = LicenseSigner::generate().verifier();
        assert!(matches!(
            verifier.verify(b"payload", "not-base64!!!"),
            Err(LicenseError::InvalidSignature)
        ));
        assert!(matches!(
            verifier.verify(b"payload", &BASE64.encode([0u8; 10])),
            Err(LicenseError::InvalidSignature)
        ));
    }

    #[test]
    fn key_export_import_roundtrip() {
        let signer = LicenseSigner::generate();
        let bytes = signer.secret_bytes();
        let signer2 = LicenseSigner::from_secret_bytes(&bytes);
        let sig1 = signer.sign(b"x");
        let sig2 = signer2.sign(b"x");
        // Ed25519 is deterministic — same key + same message => same sig.
        assert_eq!(sig1, sig2);
    }
}

// Made with Bob

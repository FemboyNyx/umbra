//! Human-comparable safety numbers.
//!
//! Uses the construction of Signal's `NumericFingerprintGenerator` (version 0):
//! `h = SHA-512(0x0000 || key || id)`, then 5200 rounds of `h = SHA-512(h || key)`;
//! the first 30 bytes become six 5-digit groups (each 5-byte big-endian chunk
//! mod 100000). Here `key = ed25519 || curve25519` and `id = PeerId bytes`.
//! The pairwise safety number is both 30-digit halves sorted and concatenated,
//! so it is identical on both devices.

use sha2::{Digest, Sha512};

const ITERATIONS: usize = 5200;
const VERSION: [u8; 2] = [0, 0];

/// 30 digits, no separators.
pub fn fingerprint_digits(ed25519: &[u8], curve25519: &[u8], peer_id_bytes: &[u8]) -> String {
    let key: Vec<u8> = [ed25519, curve25519].concat();
    let mut h = Sha512::new();
    h.update(VERSION);
    h.update(&key);
    h.update(peer_id_bytes);
    let mut hash = h.finalize();
    for _ in 0..ITERATIONS {
        let mut h = Sha512::new();
        h.update(hash);
        h.update(&key);
        hash = h.finalize();
    }
    let mut out = String::with_capacity(30);
    for chunk in hash[..30].chunks(5) {
        let mut v: u64 = 0;
        for b in chunk {
            v = (v << 8) | u64::from(*b);
        }
        out.push_str(&format!("{:05}", v % 100_000));
    }
    out
}

/// Insert a space every 5 digits.
pub fn group(digits: &str) -> String {
    digits
        .as_bytes()
        .chunks(5)
        .map(|c| std::str::from_utf8(c).unwrap_or_default())
        .collect::<Vec<_>>()
        .join(" ")
}

/// 60-digit pairwise safety number, grouped.
pub fn safety_number(ours: &str, theirs: &str) -> String {
    let (a, b) = if ours <= theirs {
        (ours, theirs)
    } else {
        (theirs, ours)
    };
    group(&format!("{a}{b}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symmetric_and_stable() {
        let a = fingerprint_digits(&[1; 32], &[2; 32], b"alice");
        let b = fingerprint_digits(&[3; 32], &[4; 32], b"bob");
        assert_eq!(a.len(), 30);
        assert!(a.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(a, fingerprint_digits(&[1; 32], &[2; 32], b"alice"));
        assert_ne!(a, b);
        assert_eq!(safety_number(&a, &b), safety_number(&b, &a));
        assert_eq!(safety_number(&a, &b).split(' ').count(), 12);
    }
}

//! Restore detection (docs/message-protection.md#anti-rollback).
//!
//! The database carries an epoch `(id, n)`. `n` goes up before every Olm
//! send and every one-time key we hand out, i.e. before anything that would
//! be unsafe to repeat from older state. The same value is written to a
//! sidecar file next to the database (outside SQLCipher, so a copied-back
//! database doesn't carry it), and the app can keep a copy in the OS keystore
//! next to `db_key` and pass it to `Core::check_epoch`.
//!
//! The database was restored from an older copy when a witness (the sidecar
//! or the keystore copy) is ahead of it: the same `id` with a larger `n`, or
//! an `id` this database has never had. A database with an epoch but no
//! sidecar is treated as restored too (the sidecar is meant to be excluded
//! from backups and device transfers).

use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context, Result};

/// Sidecar file in the data directory.
pub const EPOCH_FILE: &str = "epoch";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Epoch {
    pub id: String,
    pub n: u64,
}

impl Epoch {
    pub fn fresh() -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string(),
            n: 0,
        }
    }
}

impl fmt::Display for Epoch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.id, self.n)
    }
}

impl FromStr for Epoch {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self> {
        let (id, n) = s
            .trim()
            .split_once('.')
            .ok_or_else(|| anyhow!("bad epoch"))?;
        if id.is_empty() || id.len() > 64 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(anyhow!("bad epoch id"));
        }
        Ok(Self {
            id: id.to_string(),
            n: n.parse().context("bad epoch counter")?,
        })
    }
}

/// Is `witness` ahead of the database's `current` epoch? `history` holds the
/// ids this database had before (it moves to a new id after each recovery).
pub fn witness_is_ahead(current: &Epoch, history: &[String], witness: &Epoch) -> bool {
    if witness.id == current.id {
        witness.n > current.n
    } else {
        !history.contains(&witness.id)
    }
}

pub fn read_sidecar(dir: &Path) -> Option<Epoch> {
    std::fs::read_to_string(dir.join(EPOCH_FILE))
        .ok()?
        .parse()
        .ok()
}

/// Write the sidecar atomically (temp file + rename).
pub fn write_sidecar(dir: &Path, e: &Epoch) -> Result<()> {
    let tmp = dir.join(format!("{EPOCH_FILE}.tmp"));
    std::fs::write(&tmp, e.to_string()).context("write epoch")?;
    std::fs::rename(&tmp, dir.join(EPOCH_FILE)).context("write epoch")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_and_compare() {
        let e: Epoch = "abc123.7".parse().unwrap();
        assert_eq!(e.to_string(), "abc123.7");
        assert!("abc".parse::<Epoch>().is_err());
        assert!("a-b.1".parse::<Epoch>().is_err());
        let ahead = |w: &str, h: &[&str]| {
            let h: Vec<String> = h.iter().map(|s| s.to_string()).collect();
            witness_is_ahead(&e, &h, &w.parse().unwrap())
        };
        assert!(!ahead("abc123.7", &[]));
        assert!(!ahead("abc123.3", &[])); // db ahead of witness: crash between writes
        assert!(ahead("abc123.8", &[]));
        assert!(ahead("other.0", &[])); // an id from the future
        assert!(!ahead("old.99", &["old"])); // an id we moved past
    }
}

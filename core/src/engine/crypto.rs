//! Device identity and Olm (vodozemac) session management.
//!
//! * Ed25519 device key (libp2p `Keypair`): derives the PeerId, authenticates
//!   the Noise transport.
//! * vodozemac Olm `Account`: Curve25519 identity key + one-time/fallback keys.
//! * Olm sessions are pickled (encrypted with a key derived from the DB key via
//!   HKDF-SHA256) and stored in SQLCipher after every state change.
//!
//! No custom cryptography: Olm (3DH + double ratchet) from vodozemac, Noise
//! from libp2p, HKDF/SHA-2 from RustCrypto.

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, bail, Context, Result};
use hkdf::Hkdf;
use libp2p::identity::Keypair;
use libp2p::PeerId;
use sha2::Sha256;
use vodozemac::olm::{Account, AccountPickle, OlmMessage, Session, SessionConfig, SessionPickle};
use vodozemac::Curve25519PublicKey;
use zeroize::Zeroizing;

use crate::engine::safety;
use crate::engine::store::Store;
use crate::engine::util::{b64, now_ms};

const META_DEVICE_KEY: &str = "device_key";
const META_OLM_ACCOUNT: &str = "olm_account";
/// Sessions kept per peer.
const MAX_SESSIONS_PER_PEER: usize = 4;

/// Olm v1 (libolm-compatible). v2 is still behind vodozemac's experimental flag.
pub fn session_config() -> SessionConfig {
    SessionConfig::version_1()
}

#[derive(Clone)]
pub struct Identity {
    pub keypair: Keypair,
    pub peer_id: PeerId,
    pub ed25519: [u8; 32],
    pub curve25519: Curve25519PublicKey,
    /// Our 30-digit fingerprint half.
    pub fp_digits: String,
}

impl Identity {
    pub fn ed25519_b64(&self) -> String {
        b64(&self.ed25519)
    }
    pub fn curve25519_b64(&self) -> String {
        self.curve25519.to_base64()
    }
}

pub fn fingerprint_of(peer: &PeerId, ed25519: &[u8], curve25519: &Curve25519PublicKey) -> String {
    safety::fingerprint_digits(ed25519, curve25519.as_bytes(), &peer.to_bytes())
}

pub struct Crypto {
    account: Mutex<Account>,
    pickle_key: Zeroizing<[u8; 32]>,
    store: Arc<Store>,
}

fn derive_pickle_key(db_key: &[u8]) -> Result<Zeroizing<[u8; 32]>> {
    let hk = Hkdf::<Sha256>::new(Some(b"umbra-core"), db_key);
    let mut out = Zeroizing::new([0u8; 32]);
    hk.expand(b"olm-pickle-v1", out.as_mut())
        .map_err(|_| anyhow!("hkdf"))?;
    Ok(out)
}

impl Crypto {
    /// Load the identity from the store, or create and persist a new one.
    pub fn load_or_create(store: Arc<Store>, db_key: &[u8]) -> Result<(Identity, Self)> {
        let pickle_key = derive_pickle_key(db_key)?;

        let keypair = match store.meta_get(META_DEVICE_KEY)? {
            Some(bytes) => Keypair::from_protobuf_encoding(&Zeroizing::new(bytes))
                .context("corrupt device key")?,
            None => {
                let kp = Keypair::generate_ed25519();
                let enc = Zeroizing::new(kp.to_protobuf_encoding().context("encode device key")?);
                store.meta_set(META_DEVICE_KEY, &enc)?;
                kp
            }
        };
        let ed25519 = keypair
            .clone()
            .try_into_ed25519()
            .map_err(|_| anyhow!("device key is not ed25519"))?
            .public()
            .to_bytes();
        let peer_id = keypair.public().to_peer_id();

        let account = match store.meta_get(META_OLM_ACCOUNT)? {
            Some(bytes) => {
                let s = String::from_utf8(bytes).context("corrupt olm account")?;
                Account::from_pickle(
                    AccountPickle::from_encrypted(&s, &pickle_key).context("olm account pickle")?,
                )
            }
            None => {
                let mut acc = Account::new();
                acc.generate_fallback_key();
                store.meta_set(
                    META_OLM_ACCOUNT,
                    acc.pickle().encrypt(&pickle_key).as_bytes(),
                )?;
                acc
            }
        };
        let curve25519 = account.curve25519_key();
        let fp_digits = fingerprint_of(&peer_id, &ed25519, &curve25519);
        let id = Identity {
            keypair,
            peer_id,
            ed25519,
            curve25519,
            fp_digits,
        };
        Ok((
            id,
            Self {
                account: Mutex::new(account),
                pickle_key,
                store,
            },
        ))
    }

    fn save_account(&self, acc: &Account) -> Result<()> {
        self.store.meta_set(
            META_OLM_ACCOUNT,
            acc.pickle().encrypt(&self.pickle_key).as_bytes(),
        )
    }

    fn save_session(&self, peer: &str, s: &Session) -> Result<()> {
        self.store.save_session(
            peer,
            &s.session_id(),
            &s.pickle().encrypt(&self.pickle_key),
            now_ms(),
        )
    }

    fn load_sessions(&self, peer: &str) -> Result<Vec<Session>> {
        self.store
            .sessions_for(peer)?
            .into_iter()
            .map(|row| {
                SessionPickle::from_encrypted(&row.pickle, &self.pickle_key)
                    .map(Session::from_pickle)
                    .map_err(|e| anyhow!("session pickle: {e}"))
            })
            .collect()
    }

    /// Hand out a fresh one-time key (falls back to the fallback key).
    pub fn claim_one_time_key(&self) -> Result<Curve25519PublicKey> {
        let mut acc = self.account.lock().expect("account poisoned");
        acc.generate_one_time_keys(1);
        let key = acc.one_time_keys().into_values().next();
        let key = match key {
            Some(k) => k,
            None => match acc.fallback_key().into_values().next() {
                Some(k) => k,
                None => acc
                    .generate_fallback_key()
                    .ok_or_else(|| anyhow!("no fallback key"))?,
            },
        };
        acc.mark_keys_as_published();
        self.save_account(&acc)?;
        Ok(key)
    }

    pub fn has_session(&self, peer: &str) -> Result<bool> {
        Ok(!self.store.sessions_for(peer)?.is_empty())
    }

    pub fn create_outbound(
        &self,
        peer: &str,
        their_ik: Curve25519PublicKey,
        otk: Curve25519PublicKey,
    ) -> Result<()> {
        let acc = self.account.lock().expect("account poisoned");
        let s = acc.create_outbound_session(session_config(), their_ik, otk)?;
        self.save_session(peer, &s)?;
        self.store.prune_sessions(peer, MAX_SESSIONS_PER_PEER)?;
        Ok(())
    }

    /// Encrypt with the most recently used session. Returns (type, bytes).
    pub fn encrypt(&self, peer: &str, plaintext: &[u8]) -> Result<(u8, Vec<u8>)> {
        let _acc = self.account.lock().expect("account poisoned"); // serialize session state changes
        let mut sessions = self.load_sessions(peer)?;
        let s = sessions
            .first_mut()
            .ok_or_else(|| anyhow!("no Olm session with {peer}"))?;
        let msg = s.encrypt(plaintext)?;
        self.save_session(peer, s)?;
        let (t, bytes) = msg.to_parts();
        Ok((t as u8, bytes))
    }

    /// Decrypt a message from `peer` whose identity key we know.
    pub fn decrypt(
        &self,
        peer: &str,
        their_ik: Curve25519PublicKey,
        msg_type: u8,
        body: &[u8],
    ) -> Result<Vec<u8>> {
        let msg = OlmMessage::from_parts(msg_type as usize, body)
            .map_err(|e| anyhow!("malformed olm message: {e}"))?;
        let mut acc = self.account.lock().expect("account poisoned");
        let mut sessions = self.load_sessions(peer)?;
        match &msg {
            OlmMessage::PreKey(pre) => {
                if pre.identity_key() != their_ik {
                    bail!("pre-key message identity key does not match contact");
                }
                if let Some(s) = sessions
                    .iter_mut()
                    .find(|s| s.session_id() == pre.session_id())
                {
                    let pt = s.decrypt(&msg)?;
                    self.save_session(peer, s)?;
                    return Ok(pt);
                }
                let res = acc.create_inbound_session(session_config(), their_ik, pre)?;
                // Persist the account first: the one-time key is now consumed.
                self.save_account(&acc)?;
                self.save_session(peer, &res.session)?;
                self.store.prune_sessions(peer, MAX_SESSIONS_PER_PEER)?;
                Ok(res.plaintext)
            }
            OlmMessage::Normal(_) => {
                for s in sessions.iter_mut() {
                    if let Ok(pt) = s.decrypt(&msg) {
                        self.save_session(peer, s)?;
                        return Ok(pt);
                    }
                }
                bail!("no session could decrypt the message")
            }
        }
    }

    pub fn forget_sessions(&self, peer: &str) -> Result<()> {
        let _acc = self.account.lock().expect("account poisoned");
        self.store.delete_sessions(peer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(key: u8) -> (Identity, Crypto, Arc<Store>) {
        let k = [key; 32];
        let store = Arc::new(Store::open_in_memory(&k).unwrap());
        let (id, c) = Crypto::load_or_create(store.clone(), &k).unwrap();
        (id, c, store)
    }

    #[test]
    fn identity_persists_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("id.db");
        let k = [3u8; 32];
        let a = {
            let store = Arc::new(Store::open(&path, &k).unwrap());
            Crypto::load_or_create(store, &k).unwrap().0
        };
        let store = Arc::new(Store::open(&path, &k).unwrap());
        let b = Crypto::load_or_create(store, &k).unwrap().0;
        assert_eq!(a.peer_id, b.peer_id);
        assert_eq!(a.curve25519, b.curve25519);
        assert_eq!(a.fp_digits, b.fp_digits);
        assert_eq!(PeerId::from_public_key(&a.keypair.public()), a.peer_id);
    }

    #[test]
    fn olm_round_trip_both_ways_and_persistence() {
        let (ida, a, _sa) = node(1);
        let (idb, b, _sb) = node(2);
        let (pa, pb) = (ida.peer_id.to_base58(), idb.peer_id.to_base58());

        // A claims a key from B and opens a session.
        let otk = b.claim_one_time_key().unwrap();
        a.create_outbound(&pb, idb.curve25519, otk).unwrap();
        let (t, c) = a.encrypt(&pb, b"hello bob").unwrap();
        assert_eq!(t, 0, "first message is a pre-key message");
        assert_eq!(b.decrypt(&pa, ida.curve25519, t, &c).unwrap(), b"hello bob");

        // Replay must fail.
        assert!(b.decrypt(&pa, ida.curve25519, t, &c).is_err());

        // B replies on the inbound session; A's session becomes established.
        let (t2, c2) = b.encrypt(&pa, b"hi alice").unwrap();
        assert_eq!(t2, 1);
        assert_eq!(
            a.decrypt(&pb, idb.curve25519, t2, &c2).unwrap(),
            b"hi alice"
        );
        let (t3, c3) = a.encrypt(&pb, b"normal now").unwrap();
        assert_eq!(t3, 1);
        assert_eq!(
            b.decrypt(&pa, ida.curve25519, t3, &c3).unwrap(),
            b"normal now"
        );

        // Wrong identity key is rejected.
        let (idc, _c, _) = node(9);
        let otk = b.claim_one_time_key().unwrap();
        a.forget_sessions(&pb).unwrap();
        a.create_outbound(&pb, idb.curve25519, otk).unwrap();
        let (t4, c4) = a.encrypt(&pb, b"x").unwrap();
        assert!(b.decrypt(&pa, idc.curve25519, t4, &c4).is_err());
        assert_eq!(b.decrypt(&pa, ida.curve25519, t4, &c4).unwrap(), b"x");

        // Tampered ciphertext fails and does not break the session.
        let (t5, mut c5) = a.encrypt(&pb, b"y").unwrap();
        let last = c5.len() - 1;
        c5[last] ^= 1;
        assert!(b.decrypt(&pa, ida.curve25519, t5, &c5).is_err());
        let (t6, c6) = a.encrypt(&pb, b"z").unwrap();
        assert_eq!(b.decrypt(&pa, ida.curve25519, t6, &c6).unwrap(), b"z");
    }

    #[test]
    fn simultaneous_sessions_converge() {
        let (ida, a, _sa) = node(4);
        let (idb, b, _sb) = node(5);
        let (pa, pb) = (ida.peer_id.to_base58(), idb.peer_id.to_base58());
        a.create_outbound(&pb, idb.curve25519, b.claim_one_time_key().unwrap())
            .unwrap();
        b.create_outbound(&pa, ida.curve25519, a.claim_one_time_key().unwrap())
            .unwrap();
        let (t1, c1) = a.encrypt(&pb, b"from a").unwrap();
        let (t2, c2) = b.encrypt(&pa, b"from b").unwrap();
        assert_eq!(b.decrypt(&pa, ida.curve25519, t1, &c1).unwrap(), b"from a");
        assert_eq!(a.decrypt(&pb, idb.curve25519, t2, &c2).unwrap(), b"from b");
        for i in 0..5 {
            let m = format!("m{i}");
            let (t, c) = a.encrypt(&pb, m.as_bytes()).unwrap();
            assert_eq!(b.decrypt(&pa, ida.curve25519, t, &c).unwrap(), m.as_bytes());
            let (t, c) = b.encrypt(&pa, m.as_bytes()).unwrap();
            assert_eq!(a.decrypt(&pb, idb.curve25519, t, &c).unwrap(), m.as_bytes());
        }
    }
}

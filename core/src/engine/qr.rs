//! `umbra://add?peer=<PeerId>&ik=<curve25519>&ed=<ed25519>&addr=<multiaddrs>[&name=<display name>]`
//!
//! * `ik`, `ed`: unpadded standard base64 (URL-encoded in the query string).
//! * `addr`: may appear several times; each value may also be a
//!   comma-separated list of multiaddrs.
//! * `name`: optional display name.
//! * `sig`: card v2 signature (unpadded base64), see [`card_sig_message`].
//!   Optional for compatibility with v1 cards; verified when present.

use anyhow::{anyhow, bail, Context, Result};
use libp2p::{identity, Multiaddr, PeerId};
use url::Url;

use crate::api::types::ContactCard;
use crate::engine::util::{b64, unb64};

/// Domain separator of the card v2 signature.
pub const CARD_SIG_CONTEXT: &[u8] = b"umbra-card-v2";

/// `"umbra-card-v2" || PeerId bytes (multihash) || ik (32 raw bytes)`.
/// No length prefixes needed: the multihash is self-delimiting and `ik` is
/// fixed-size.
pub fn card_sig_message(peer: &PeerId, ik: &vodozemac::Curve25519PublicKey) -> Vec<u8> {
    let mut m = CARD_SIG_CONTEXT.to_vec();
    m.extend_from_slice(&peer.to_bytes());
    m.extend_from_slice(ik.as_bytes());
    m
}

/// Sign our own card (v2) with the device key.
pub fn sign_card(keypair: &identity::Keypair, card: &mut ContactCard) -> Result<()> {
    let peer: PeerId = card.peer_id.parse().context("invalid PeerId")?;
    let ik = vodozemac::Curve25519PublicKey::from_base64(card.identity_key.trim_end_matches('='))
        .map_err(|e| anyhow!("invalid curve25519 identity key: {e}"))?;
    let sig = keypair
        .sign(&card_sig_message(&peer, &ik))
        .map_err(|e| anyhow!("sign card: {e}"))?;
    card.sig = Some(b64(&sig));
    Ok(())
}

pub const SCHEME: &str = "umbra";
/// Keep QR codes scannable.
pub const MAX_QR_ADDRS: usize = 6;

pub fn build_qr(card: &ContactCard) -> String {
    let mut url = Url::parse("umbra://add").expect("static url");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("peer", &card.peer_id);
        q.append_pair("ik", &card.identity_key);
        q.append_pair("ed", &card.ed25519_key);
        if let Some(sig) = &card.sig {
            q.append_pair("sig", sig);
        }
        if !card.addrs.is_empty() {
            let addrs: Vec<&str> = card
                .addrs
                .iter()
                .take(MAX_QR_ADDRS)
                .map(String::as_str)
                .collect();
            q.append_pair("addr", &addrs.join(","));
        }
        if let Some(name) = card.display_name.as_deref().filter(|n| !n.is_empty()) {
            q.append_pair("name", name);
        }
    }
    url.to_string()
}

pub fn parse_qr(payload: &str) -> Result<ContactCard> {
    let url = Url::parse(payload.trim()).context("not a valid umbra:// link")?;
    if url.scheme() != SCHEME || url.host_str() != Some("add") {
        bail!("not an umbra://add link");
    }
    let (mut peer, mut ik, mut ed, mut name, mut sig) = (None, None, None, None, None);
    let mut addrs = Vec::new();
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "peer" => peer = Some(v.into_owned()),
            "ik" => ik = Some(v.into_owned()),
            "ed" => ed = Some(v.into_owned()),
            "name" => name = Some(v.into_owned()),
            "sig" => sig = Some(v.into_owned()),
            "addr" => {
                for a in v.split(',').map(str::trim).filter(|a| !a.is_empty()) {
                    let ma: Multiaddr = a.parse().with_context(|| format!("bad multiaddr {a}"))?;
                    addrs.push(ma.to_string());
                }
            }
            _ => {} // forward compatible: ignore unknown params
        }
    }
    let card = ContactCard {
        peer_id: peer.ok_or_else(|| anyhow!("missing peer"))?,
        identity_key: ik.ok_or_else(|| anyhow!("missing ik"))?,
        ed25519_key: ed.ok_or_else(|| anyhow!("missing ed"))?,
        addrs,
        display_name: name.filter(|n| !n.is_empty()),
        sig,
    };
    validate_card(&card)?;
    Ok(card)
}

/// Check that keys decode, that `ed` really derives `peer`, and (card v2)
/// that `sig` is a valid signature by `ed` over `peer` and `ik`. A card with
/// an invalid signature is rejected; an unsigned (v1) card passes here, so
/// callers that need v2 must also check [`ContactCard::sig`] is present.
pub fn validate_card(card: &ContactCard) -> Result<PeerId> {
    let peer: PeerId = card.peer_id.parse().context("invalid PeerId")?;
    let ed = unb64(&card.ed25519_key)?;
    let ed = identity::ed25519::PublicKey::try_from_bytes(&ed).context("invalid ed25519 key")?;
    let derived = PeerId::from_public_key(&identity::PublicKey::from(ed.clone()));
    if derived != peer {
        bail!("ed25519 key does not match PeerId");
    }
    let ik = vodozemac::Curve25519PublicKey::from_base64(card.identity_key.trim_end_matches('='))
        .map_err(|e| anyhow!("invalid curve25519 identity key: {e}"))?;
    if let Some(sig) = &card.sig {
        let sig = unb64(sig).context("invalid card signature")?;
        if !ed.verify(&card_sig_message(&peer, &ik), &sig) {
            bail!("invalid card signature");
        }
    }
    for a in &card.addrs {
        a.parse::<Multiaddr>()
            .with_context(|| format!("bad multiaddr {a}"))?;
    }
    Ok(peer)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signed_card() -> (ContactCard, identity::Keypair) {
        let kp = identity::Keypair::generate_ed25519();
        let ed = kp.public().try_into_ed25519().unwrap();
        let ik = vodozemac::olm::Account::new().curve25519_key();
        let mut card = ContactCard {
            peer_id: kp.public().to_peer_id().to_base58(),
            identity_key: ik.to_base64(),
            ed25519_key: b64(&ed.to_bytes()),
            addrs: vec![],
            display_name: Some("x".into()),
            sig: None,
        };
        sign_card(&kp, &mut card).unwrap();
        (card, kp)
    }

    #[test]
    fn card_v2_signature_round_trips_and_is_enforced() {
        let (card, _) = signed_card();
        validate_card(&card).unwrap();
        // Survives the QR round trip.
        let parsed = parse_qr(&build_qr(&card)).unwrap();
        assert_eq!(parsed.sig, card.sig);

        // Another identity key under the same signature is rejected.
        let other = vodozemac::olm::Account::new().curve25519_key();
        let swapped = ContactCard {
            identity_key: other.to_base64(),
            ..card.clone()
        };
        assert!(validate_card(&swapped).is_err());
        // A signature by another device key is rejected.
        let (mut foreign, _) = signed_card();
        foreign.sig = card.sig.clone();
        assert!(validate_card(&foreign).is_err());
        // Garbage is rejected; a v1 (unsigned) card still parses.
        let garbage = ContactCard {
            sig: Some("AAAA".into()),
            ..card.clone()
        };
        assert!(validate_card(&garbage).is_err());
        let v1 = ContactCard {
            sig: None,
            ..card.clone()
        };
        validate_card(&v1).unwrap();
        assert!(parse_qr(&build_qr(&v1)).unwrap().sig.is_none());
    }

    #[test]
    fn signed_message_layout_is_stable() {
        let (card, _) = signed_card();
        let peer: PeerId = card.peer_id.parse().unwrap();
        let ik = vodozemac::Curve25519PublicKey::from_base64(&card.identity_key).unwrap();
        let m = card_sig_message(&peer, &ik);
        assert!(m.starts_with(b"umbra-card-v2"));
        assert_eq!(m.len(), 13 + peer.to_bytes().len() + 32);
        assert!(m.ends_with(ik.as_bytes()));
    }
}

//! `umbra://add?peer=<PeerId>&ik=<curve25519>&ed=<ed25519>&addr=<multiaddrs>[&name=<display name>]`
//!
//! * `ik`, `ed`: unpadded standard base64 (URL-encoded in the query string).
//! * `addr`: may appear several times; each value may also be a
//!   comma-separated list of multiaddrs.
//! * `name`: optional display name.

use anyhow::{anyhow, bail, Context, Result};
use libp2p::{identity, Multiaddr, PeerId};
use url::Url;

use crate::api::types::ContactCard;
use crate::engine::util::unb64;

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
    let (mut peer, mut ik, mut ed, mut name) = (None, None, None, None);
    let mut addrs = Vec::new();
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "peer" => peer = Some(v.into_owned()),
            "ik" => ik = Some(v.into_owned()),
            "ed" => ed = Some(v.into_owned()),
            "name" => name = Some(v.into_owned()),
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
    };
    validate_card(&card)?;
    Ok(card)
}

/// Check that keys decode and that `ed` really derives `peer`.
pub fn validate_card(card: &ContactCard) -> Result<PeerId> {
    let peer: PeerId = card.peer_id.parse().context("invalid PeerId")?;
    let ed = unb64(&card.ed25519_key)?;
    let ed = identity::ed25519::PublicKey::try_from_bytes(&ed).context("invalid ed25519 key")?;
    let derived = PeerId::from_public_key(&identity::PublicKey::from(ed));
    if derived != peer {
        bail!("ed25519 key does not match PeerId");
    }
    vodozemac::Curve25519PublicKey::from_base64(card.identity_key.trim_end_matches('='))
        .map_err(|e| anyhow!("invalid curve25519 identity key: {e}"))?;
    for a in &card.addrs {
        a.parse::<Multiaddr>()
            .with_context(|| format!("bad multiaddr {a}"))?;
    }
    Ok(peer)
}

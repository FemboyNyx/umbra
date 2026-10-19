//! Two in-process nodes on localhost: add each other, establish Olm,
//! exchange messages both ways, pass a call signal, and queue while offline.
//!
//! With `--features tor` the swarm is always Tor-only (embedded arti), so the
//! clearnet tests are compiled out; the SOCKS Tor-mode test runs in both.
#![cfg_attr(feature = "tor", allow(dead_code))]

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::timeout;
use umbra_core::api::types::*;
use umbra_core::engine::core::Core;
use umbra_core::engine::events::EventHub;

const T: Duration = Duration::from_secs(20);

struct Node {
    core: Arc<Core>,
    rx: UnboundedReceiver<CoreEvent>,
    _dir: tempfile::TempDir,
}

fn cfg() -> CoreConfig {
    CoreConfig {
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".into()],
        enable_mdns: false,
        ..Default::default()
    }
}

async fn node(key: u8, name: &str) -> Node {
    let dir = tempfile::tempdir().unwrap();
    let hub = EventHub::new();
    let rx = hub.subscribe();
    let core = Core::open(dir.path(), &[key; 32], cfg(), hub).unwrap();
    core.set_display_name(name.into()).unwrap();
    core.start_network().await.unwrap();
    let mut n = Node {
        core,
        rx,
        _dir: dir,
    };
    wait_for(
        &mut n,
        |e| matches!(e, CoreEvent::NetworkStatus { info } if !info.listen_addrs.is_empty()),
    )
    .await;
    n
}

async fn wait_for(n: &mut Node, mut f: impl FnMut(&CoreEvent) -> bool) -> CoreEvent {
    timeout(T, async {
        loop {
            let ev = n.rx.recv().await.expect("event stream closed");
            if f(&ev) {
                return ev;
            }
        }
    })
    .await
    .expect("timed out waiting for event")
}

fn init_tracing() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
}

#[cfg(not(feature = "tor"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn two_nodes_chat_and_signal() {
    init_tracing();
    let mut a = node(1, "Alice").await;
    let mut b = node(2, "Bob").await;

    let qa = a.core.self_info();
    let qb = b.core.self_info();
    assert!(
        !qb.addrs.is_empty(),
        "loopback addr advertised when nothing else"
    );
    let card_b = umbra_core::engine::qr::parse_qr(&qb.qr_payload).unwrap();
    assert_eq!(card_b.display_name.as_deref(), Some("Bob"));

    // Alice scans Bob. Bob doesn't know Alice yet -> ContactRequest.
    let cb = a.core.add_contact(card_b, true).await.unwrap();
    let req = wait_for(&mut b, |e| matches!(e, CoreEvent::ContactRequest { .. })).await;
    let CoreEvent::ContactRequest { card: card_a } = req else {
        unreachable!()
    };
    assert_eq!(card_a.peer_id, qa.peer_id);
    assert_eq!(card_a.display_name.as_deref(), Some("Alice"));

    // Alice queues a message before Bob accepts.
    let m1 = a
        .core
        .send_text(&qb.peer_id, "hello bob".into())
        .await
        .unwrap();

    // Bob accepts -> Alice's outbox flushes.
    let ca = b.core.add_contact(card_a, false).await.unwrap();
    assert_eq!(
        ca.safety_number, cb.safety_number,
        "safety numbers match on both sides"
    );

    let ev = wait_for(&mut b, |e| matches!(e, CoreEvent::MessageReceived { .. })).await;
    let CoreEvent::MessageReceived { message } = ev else {
        unreachable!()
    };
    assert_eq!(message.body, "hello bob");
    assert_eq!(message.id, m1);
    assert_eq!(message.peer_id, qa.peer_id);
    wait_for(&mut a, |e| matches!(e, CoreEvent::MessageStatus { id, status: DeliveryStatus::Delivered, .. } if *id == m1)).await;

    // Bob replies (inbound session -> normal messages from here on).
    let m2 = b
        .core
        .send_text(&qa.peer_id, "hi alice".into())
        .await
        .unwrap();
    let ev = wait_for(&mut a, |e| matches!(e, CoreEvent::MessageReceived { .. })).await;
    let CoreEvent::MessageReceived { message } = ev else {
        unreachable!()
    };
    assert_eq!(message.body, "hi alice");
    wait_for(&mut b, |e| matches!(e, CoreEvent::MessageStatus { id, status: DeliveryStatus::Delivered, .. } if *id == m2)).await;

    // A burst both ways.
    for i in 0..5 {
        a.core
            .send_text(&qb.peer_id, format!("a{i}"))
            .await
            .unwrap();
        b.core
            .send_text(&qa.peer_id, format!("b{i}"))
            .await
            .unwrap();
    }
    for i in 0..5 {
        let want = format!("a{i}");
        wait_for(
            &mut b,
            |e| matches!(e, CoreEvent::MessageReceived { message } if message.body == want),
        )
        .await;
        let want = format!("b{i}");
        wait_for(
            &mut a,
            |e| matches!(e, CoreEvent::MessageReceived { message } if message.body == want),
        )
        .await;
    }

    // Call signaling.
    a.core
        .send_signal(
            &qb.peer_id,
            "call-1".into(),
            SignalKind::Offer,
            "v=0 sdp".into(),
        )
        .await
        .unwrap();
    let ev = wait_for(&mut b, |e| matches!(e, CoreEvent::Signal { .. })).await;
    assert_eq!(
        ev,
        CoreEvent::Signal {
            peer_id: qa.peer_id.clone(),
            call_id: "call-1".into(),
            kind: SignalKind::Offer,
            payload: "v=0 sdp".into()
        }
    );
    b.core
        .send_signal(
            &qa.peer_id,
            "call-1".into(),
            SignalKind::Answer,
            "answer".into(),
        )
        .await
        .unwrap();
    wait_for(&mut a, |e| {
        matches!(
            e,
            CoreEvent::Signal {
                kind: SignalKind::Answer,
                ..
            }
        )
    })
    .await;

    // Stored history and read state.
    let hist = b.core.list_messages(&qa.peer_id, None, 100).unwrap();
    assert_eq!(hist.len(), 12);
    let conv = b.core.list_conversations().unwrap();
    assert_eq!(conv[0].unread_count, 6);
    b.core.mark_read(&qa.peer_id, &hist[0].id).unwrap();
    assert_eq!(b.core.list_conversations().unwrap()[0].unread_count, 0);
    let contacts = a.core.list_contacts().unwrap();
    assert!(contacts[0].verified && contacts[0].online);

    a.core.stop_network().await.unwrap();
    b.core.stop_network().await.unwrap();
}

/// Servers Phase 0: two co-members who are NOT contacts exchange a queued
/// server payload over the real network (Hello, key claim, Olm, outbox),
/// with the optional envelope fields intact.
#[cfg(not(feature = "tor"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn co_members_deliver_server_payloads_via_outbox() {
    use umbra_core::engine::proto::EnvelopeMeta;
    use umbra_core::engine::servers::{ServerIdentity, ServerPayloadKind};
    init_tracing();
    let a = node(1, "Alice").await;
    let b = node(2, "Bob").await;
    let mut inbound = b.core.subscribe_server();
    for n in [&a, &b] {
        n.core
            .add_server_membership("srv1", ServerIdentity::Default, None)
            .unwrap();
    }
    let card = |n: &Node| umbra_core::engine::qr::parse_qr(&n.core.self_info().qr_payload).unwrap();
    a.core.add_co_member("srv1", card(&b)).unwrap();
    b.core.add_co_member("srv1", card(&a)).unwrap();
    let meta = EnvelopeMeta {
        seq: Some(7),
        prev: Some("cHJldg".into()),
        expires_at: Some(4_102_444_800_000),
    };
    let id = a
        .core
        .queue_server_payload(
            &b.core.peer_id().to_base58(),
            "srv1",
            ServerPayloadKind::ServerKey,
            b"session-key",
            meta.clone(),
        )
        .unwrap();
    let got = timeout(T, inbound.recv()).await.unwrap().unwrap();
    assert_eq!(got.id, id);
    assert_eq!(got.peer_id, a.core.peer_id().to_base58());
    assert_eq!(got.kind, ServerPayloadKind::ServerKey);
    assert_eq!(got.data, b"session-key");
    assert_eq!(got.meta, meta);
    assert!(a.core.list_contacts().unwrap().is_empty());
    assert!(b.core.list_contacts().unwrap().is_empty());
}

#[cfg(not(feature = "tor"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn invite_join_and_kick_over_the_network() {
    init_tracing();
    let mut a = node(11, "Alice").await;
    let mut b = node(12, "Bob").await;
    let sid = a
        .core
        .create_server("Den", vec![], Some("Owner Al".into()))
        .unwrap()
        .server
        .server_id;
    let link = a
        .core
        .create_invite(&sid, InviteOptions::default())
        .unwrap()
        .link;
    // Strangers: no contact relationship, only the invite.
    let info = b
        .core
        .join_server(&link, Some("Bobby".into()))
        .await
        .unwrap();
    assert_eq!(info.status, ServerStatus::Joining);
    wait_for(&mut b, |e| {
        matches!(e, CoreEvent::Server { event: ServerEvent::Joined, server_id } if *server_id == sid)
    })
    .await;
    let bi = b.core.server_info(&sid).unwrap();
    assert_eq!((bi.status, bi.member_count), (ServerStatus::Active, 2));
    assert!(bi.history_shared, "history is shared by default");
    let names: Vec<String> = a
        .core
        .list_members(&sid)
        .unwrap()
        .into_iter()
        .map(|m| m.display_name)
        .collect();
    assert!(names.contains(&"Bobby".to_string()));
    assert!(
        a.core.list_contacts().unwrap().is_empty(),
        "joining makes no contacts"
    );

    // A used invite is refused over the wire too.
    let mut c = node(13, "Carol").await;
    let err = c.core.join_server(&link, None).await.unwrap_err();
    assert!(format!("{err:#}").contains("InviteInvalid"), "{err:#}");

    // The owner creates a channel and then kicks Bob; Bob is told directly.
    a.core.create_channel(&sid, "news").unwrap();
    wait_for(&mut b, |e| {
        matches!(
            e,
            CoreEvent::Server {
                event: ServerEvent::Updated { seq: 2 },
                ..
            }
        )
    })
    .await;
    assert_eq!(b.core.list_channels(&sid).unwrap().len(), 2);
    a.core.kick(&sid, &b.core.peer_id().to_base58()).unwrap();
    wait_for(&mut b, |e| {
        matches!(
            e,
            CoreEvent::Server {
                event: ServerEvent::Removed {
                    reason: MemberLeftReason::Kicked
                },
                ..
            }
        )
    })
    .await;
    assert_eq!(
        b.core.server_info(&sid).unwrap().status,
        ServerStatus::Removed
    );
    let _ = &mut c;
}

#[cfg(not(feature = "tor"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn outbox_survives_offline_peer() {
    init_tracing();
    let mut a = node(3, "A").await;
    let mut b = node(4, "B").await;
    let (qa, qb) = (a.core.self_info(), b.core.self_info());
    a.core
        .add_contact(
            umbra_core::engine::qr::parse_qr(&qb.qr_payload).unwrap(),
            false,
        )
        .await
        .unwrap();
    b.core
        .add_contact(
            umbra_core::engine::qr::parse_qr(&qa.qr_payload).unwrap(),
            false,
        )
        .await
        .unwrap();
    let id = a.core.send_text(&qb.peer_id, "first".into()).await.unwrap();
    wait_for(&mut a, |e| matches!(e, CoreEvent::MessageStatus { id: i, status: DeliveryStatus::Delivered, .. } if *i == id)).await;

    // B goes offline; A queues.
    b.core.stop_network().await.unwrap();
    let id2 = a
        .core
        .send_text(&qb.peer_id, "while you were away".into())
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(500)).await;
    let m = a.core.list_messages(&qb.peer_id, None, 1).unwrap();
    assert_eq!(m[0].status, DeliveryStatus::Sent);

    // B comes back on a new port and dials A (as if it re-added / got discovered).
    b.core.start_network().await.unwrap();
    timeout(T, async {
        while b.core.network_info().listen_addrs.is_empty() {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let qb2 = b.core.self_info();
    // A learns B's new address (QR re-scan / mDNS / DHT in real life).
    a.core
        .add_contact(
            umbra_core::engine::qr::parse_qr(&qb2.qr_payload).unwrap(),
            false,
        )
        .await
        .unwrap();
    wait_for(&mut b, |e| matches!(e, CoreEvent::MessageReceived { message } if message.body == "while you were away")).await;
    wait_for(&mut a, |e| matches!(e, CoreEvent::MessageStatus { id: i, status: DeliveryStatus::Delivered, .. } if *i == id2)).await;
}

/// Tor mode: no listeners, and dials go to the SOCKS proxy (not direct).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn tor_mode_is_socks_only() {
    use tokio::io::AsyncReadExt;
    init_tracing();
    let proxy = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_addr = proxy.local_addr().unwrap();

    // B only needs a card with a dialable address; its network stays off so
    // the test never touches the real Tor network in `--features tor` builds.
    let bdir = tempfile::tempdir().unwrap();
    let b = Core::open(bdir.path(), &[6; 32], cfg(), EventHub::new()).unwrap();
    let mut card_b = b.self_card();
    card_b.addrs = vec!["/ip4/127.0.0.1/tcp/9".into()];

    let dir = tempfile::tempdir().unwrap();
    let hub = EventHub::new();
    let cfg = CoreConfig {
        tor_socks: Some(proxy_addr.to_string()),
        enable_mdns: true, // must be ignored in Tor mode
        listen_addrs: vec!["/ip4/127.0.0.1/tcp/0".into()],
        ..Default::default()
    };
    let a = Core::open(dir.path(), &[5; 32], cfg, hub).unwrap();
    a.start_network().await.unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        a.network_info().listen_addrs.is_empty(),
        "Tor mode must not listen on clearnet"
    );
    assert!(a.self_info().addrs.is_empty());

    a.add_contact(card_b, false).await.unwrap();
    let (mut s, _) = timeout(T, proxy.accept())
        .await
        .expect("dial must go through the SOCKS proxy")
        .unwrap();
    let mut greet = [0u8; 3];
    s.read_exact(&mut greet).await.unwrap();
    assert_eq!(greet, [5, 1, 0], "SOCKS5 greeting");
    a.stop_network().await.unwrap();
}

/// A device whose whole data dir (database and epoch sidecar) is rolled back
/// can't tell by itself. Its peer can: it has already seen that `seq` with
/// other content. The peer rejects it as `Replayed`; the sender drops its
/// sessions, jumps its sequence numbers and resends on a fresh session.
#[cfg(not(feature = "tor"))]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rolled_back_sender_is_caught_by_its_peer_and_recovers() {
    init_tracing();
    let a_dir = tempfile::tempdir().unwrap();
    let backup = tempfile::tempdir().unwrap();
    let start = |key: u8| {
        let dir = a_dir.path().to_path_buf();
        async move {
            let hub = EventHub::new();
            let rx = hub.subscribe();
            let core = Core::open(&dir, &[key; 32], cfg(), hub).unwrap();
            core.start_network().await.unwrap();
            (core, rx)
        }
    };
    let stop = |core: Arc<Core>| async move {
        core.stop_network().await.unwrap();
        for _ in 0..250 {
            if Arc::strong_count(&core) == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        drop(core);
    };
    let copy_dir = |from: &std::path::Path, to: &std::path::Path| {
        for e in std::fs::read_dir(to).unwrap() {
            let p = e.unwrap().path();
            if p.is_file() {
                std::fs::remove_file(p).unwrap();
            }
        }
        for e in std::fs::read_dir(from).unwrap() {
            let p = e.unwrap().path();
            if p.is_file() {
                std::fs::copy(&p, to.join(p.file_name().unwrap())).unwrap();
            }
        }
    };

    let mut b = node(6, "B").await;
    let qb = b.core.self_info();
    let (a, _rx) = start(5).await;
    let qa = a.self_info();
    let card = |q: &SelfInfo| umbra_core::engine::qr::parse_qr(&q.qr_payload).unwrap();
    a.add_contact(card(&qb), false).await.unwrap();
    b.core.add_contact(card(&qa), false).await.unwrap();

    // Back up A's whole data dir (database and sidecar) while it is closed.
    stop(a).await;
    copy_dir(a_dir.path(), backup.path());

    let (a, _rx) = start(5).await;
    a.send_text(&qb.peer_id, "first".into()).await.unwrap();
    wait_for(
        &mut b,
        |e| matches!(e, CoreEvent::MessageReceived { message } if message.body == "first"),
    )
    .await;

    // Roll A back to the backup, which never sent "first".
    stop(a).await;
    copy_dir(backup.path(), a_dir.path());
    let (a, mut arx) = start(5).await;
    assert_eq!(a.last_restore_ms().unwrap(), None, "undetectable locally");
    let id = a.send_text(&qb.peer_id, "second".into()).await.unwrap();
    wait_for(
        &mut b,
        |e| matches!(e, CoreEvent::MessageReceived { message } if message.body == "second"),
    )
    .await;
    timeout(T, async {
        loop {
            if let Some(CoreEvent::MessageStatus {
                id: i,
                status: DeliveryStatus::Delivered,
                ..
            }) = arx.recv().await
            {
                if i == id {
                    return;
                }
            }
        }
    })
    .await
    .expect("delivered after the jump");
    // B shows each message once.
    let msgs = b.core.list_messages(&qa.peer_id, None, 10).unwrap();
    assert_eq!(msgs.len(), 2);
    a.stop_network().await.unwrap();
    b.core.stop_network().await.unwrap();
}

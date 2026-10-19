import 'package:flutter_test/flutter_test.dart';
import 'package:umbra/src/core/qr.dart';
import 'package:umbra/src/models/core_types.dart';

void main() {
  const card = ContactCard(
    peerId: '12D3KooWPeer',
    identityKey: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA',
    ed25519Key: 'BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB',
    addrs: ['/ip4/192.0.2.1/tcp/4001/p2p/12D3KooWPeer'],
    displayName: 'Nyx',
  );

  test('round-trips an umbra://add link', () {
    final parsed = parseUmbraQr(buildUmbraQr(card));
    expect(parsed.peerId, card.peerId);
    expect(parsed.identityKey, card.identityKey);
    expect(parsed.ed25519Key, card.ed25519Key);
    expect(parsed.addrs, card.addrs);
    expect(parsed.displayName, 'Nyx');
  });

  test('rejects a link that is not umbra://add', () {
    expect(
      () => parseUmbraQr('https://example.com/add'),
      throwsA(isA<UmbraException>()),
    );
  });

  test('keeps at most six addresses', () {
    final addrs = [for (var i = 0; i < 8; i++) '/ip4/192.0.2.$i/tcp/1'];
    final payload = buildUmbraQr(
      ContactCard(
        peerId: card.peerId,
        identityKey: card.identityKey,
        ed25519Key: card.ed25519Key,
        addrs: addrs,
      ),
    );
    expect(parseUmbraQr(payload).addrs, hasLength(6));
  });
}

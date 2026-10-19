import '../models/core_types.dart';

/// Structural parse of `umbra://add?...`.
///
/// The real core additionally checks that `ed` derives `peer` and that `ik`
/// is a Curve25519 key. [FrbUmbraCore] calls that check. This parser is what
/// the mock backend and tests use, and it works before `init`.
ContactCard parseUmbraQr(String payload) {
  final Uri uri;
  try {
    uri = Uri.parse(payload.trim());
  } on FormatException {
    throw const UmbraException('not a valid umbra:// link');
  }
  if (uri.scheme != 'umbra' || uri.host != 'add') {
    throw const UmbraException('not an umbra://add link');
  }

  final params = uri.queryParametersAll;
  final peer = _first(params['peer']);
  // Form decoding turns a raw '+' into a space. Standard base64 never
  // contains a space, so putting '+' back is safe. ik and ed stay standard
  // base64 (vodozemac rejects url-safe). sig may be url-safe; core accepts
  // that, and we do not rewrite '-' or '_'.
  final ik = _plusFromSpace(_first(params['ik']));
  final ed = _plusFromSpace(_first(params['ed']));
  if (peer == null || peer.isEmpty) {
    throw const UmbraException('missing peer');
  }
  if (ik == null || ik.isEmpty) {
    throw const UmbraException('missing ik');
  }
  if (ed == null || ed.isEmpty) {
    throw const UmbraException('missing ed');
  }
  if (!_looksLikeBase64(ik) || !_looksLikeBase64(ed)) {
    throw const UmbraException('identity keys must be base64');
  }

  final addrs = <String>[];
  for (final value in params['addr'] ?? const <String>[]) {
    for (final part in value.split(',')) {
      final addr = part.trim();
      if (addr.isEmpty) continue;
      if (!addr.startsWith('/')) {
        throw UmbraException('bad multiaddr $addr');
      }
      addrs.add(addr);
    }
  }
  if (addrs.length > 6) {
    addrs.removeRange(6, addrs.length);
  }

  final name = _first(params['name']);
  final sig = _plusFromSpace(_first(params['sig']));
  return ContactCard(
    peerId: peer,
    identityKey: ik,
    ed25519Key: ed,
    addrs: addrs,
    displayName: (name == null || name.isEmpty) ? null : name,
    sig: (sig == null || sig.isEmpty) ? null : sig,
  );
}

String buildUmbraQr(ContactCard card) {
  final query = <String, String>{
    'peer': card.peerId,
    'ik': card.identityKey,
    'ed': card.ed25519Key,
  };
  if (card.addrs.isNotEmpty) {
    query['addr'] = card.addrs.take(6).join(',');
  }
  final name = card.displayName?.trim();
  if (name != null && name.isNotEmpty) {
    query['name'] = name;
  }
  if (card.sig != null) {
    query['sig'] = card.sig!;
  }
  return Uri(scheme: 'umbra', host: 'add', queryParameters: query).toString();
}

String? _first(List<String>? values) {
  if (values == null || values.isEmpty) return null;
  return values.first;
}

String? _plusFromSpace(String? value) => value?.replaceAll(' ', '+');

bool _looksLikeBase64(String value) {
  return RegExp(r'^[A-Za-z0-9+/]+={0,2}$').hasMatch(value);
}

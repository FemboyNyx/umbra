import 'package:flutter/foundation.dart';

import '../models/core_types.dart';

/// Shown when `start_network` fails. The raw error can include listen
/// addresses, including a `.onion` later, so it never reaches the UI.
const couldNotStartNetwork = "couldn't start network";

bool coreErrorMayContainAddress(String raw) {
  final lower = raw.toLowerCase();
  return lower.contains('/ip4/') ||
      lower.contains('/ip6/') ||
      lower.contains('/dns') ||
      lower.contains('.onion') ||
      lower.contains('/onion') ||
      lower.contains('multiaddr') ||
      lower.contains('could not listen');
}

/// User-facing text for startup. Address-bearing core errors collapse to
/// [couldNotStartNetwork].
String startupErrorMessage(Object err) {
  if (err is UmbraException && err.message == couldNotStartNetwork) {
    return couldNotStartNetwork;
  }
  final raw = err.toString();
  if (coreErrorMayContainAddress(raw)) return couldNotStartNetwork;
  if (err is UmbraException) return err.message;
  return raw;
}

/// Debug-only. Core errors may embed our listen addresses, so they must not
/// be logged at info level or higher.
void debugLogCoreError(Object error, [StackTrace? stack]) {
  if (!kDebugMode) return;
  final trace = stack == null ? '' : '\n$stack';
  debugPrint('core: $error$trace');
}

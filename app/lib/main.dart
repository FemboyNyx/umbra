import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:path_provider/path_provider.dart';
import 'package:umbra_calls/umbra_calls.dart' as calls;

import 'src/calls/calls_bridge.dart';
import 'src/calls/controller_call_client.dart';
import 'src/calls/outgoing_call.dart';
import 'src/calls/webrtc_audio_probe.dart';
import 'src/core/core_factory.dart';
import 'src/keystore/db_key_store.dart';
import 'src/settings/settings_store.dart';
import 'src/state/umbra_controller.dart';
import 'src/ui/umbra_app.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final settings = await SettingsStore.load();
  final controller = UmbraController(
    core: createUmbraCore(mock: settings.mock),
    keyStore: PlatformDbKeyStore(),
    settings: settings,
    supportDirectory: () async {
      final dir = await getApplicationSupportDirectory();
      return dir.path;
    },
  );
  UmbraCallsBridge? bridge;
  StreamSubscription<calls.MissedCall>? missed;
  controller.attachCalls = (self) {
    missed?.cancel();
    bridge = UmbraCallsBridge(
      core: controller.core,
      localPeerId: self.peerId,
      torMode: controller.settings.torSocks.trim().isNotEmpty,
    );
    controller.handleCoreSignal = bridge!.handleCoreSignal;
    final outgoing = ControllerOutgoingCall(bridge!.controller);
    outgoing.onIdle = controller.noteCallEnded;
    // The probe is not invoked here. WebRTC/ADM stays deferred until Start call.
    controller.outgoing = OutgoingCallGuard(
      client: outgoing,
      probe: const WebrtcAudioProbe(),
    );
    controller.setCallsBlockedByTor(bridge!.controller.callsBlockedByTor);
    missed = bridge!.controller.missedCalls.listen((call) {
      controller.noteMissedCall(
        peerId: call.peerId,
        callId: call.callId,
        reason: call.reason,
      );
    });
  };
  controller.applyTorMode = (enabled) {
    final current = bridge;
    if (current == null) return;
    current.setTorMode(enabled);
    controller.setCallsBlockedByTor(current.controller.callsBlockedByTor);
  };
  runApp(UmbraApp(controller: controller));
}

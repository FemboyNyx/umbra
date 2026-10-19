import 'package:flutter/widgets.dart';
import 'package:path_provider/path_provider.dart';

import 'src/calls/calls_bridge.dart';
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
  UmbraCallsBridge? calls;
  controller.attachCalls = (self) {
    calls = UmbraCallsBridge(core: controller.core, localPeerId: self.peerId);
    controller.handleCoreSignal = calls!.handleCoreSignal;
  };
  runApp(UmbraApp(controller: controller));
}

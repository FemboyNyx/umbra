package io.github.femboynyx.umbra

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
            .setMethodCallHandler { call, result ->
                if (call.method != "getOrCreateDbKey") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                if (call.argument<String>("storageDir") == null) {
                    result.error("keystore", "storageDir is required", null)
                    return@setMethodCallHandler
                }
                try {
                    result.success(UmbraKeystore(this).getOrCreate())
                } catch (err: Exception) {
                    result.error("keystore", err.message, null)
                }
            }
    }

    companion object {
        private const val CHANNEL = "io.github.femboynyx.umbra/keystore"
    }
}

package io.github.femboynyx.umbra

import android.annotation.SuppressLint
import android.app.Activity
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import android.view.WindowManager
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.util.concurrent.atomic.AtomicBoolean

class MainActivity : FlutterActivity() {
    private var capture: CaptureHooks? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val messenger = flutterEngine.dartExecutor.binaryMessenger
        MethodChannel(messenger, KEYSTORE_CHANNEL).setMethodCallHandler { call, result ->
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
        MethodChannel(messenger, SCREEN_CHANNEL).setMethodCallHandler { call, result ->
            if (call.method != "setSecure") {
                result.notImplemented()
                return@setMethodCallHandler
            }
            val enabled = call.argument<Boolean>("enabled") == true
            if (enabled) {
                window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            } else {
                window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
            }
            result.success(mapOf("native" to true))
        }
        MethodChannel(messenger, BIOMETRIC_CHANNEL).setMethodCallHandler { call, result ->
            when (call.method) {
                "available" -> result.success(biometricsAvailable())
                "authenticate" -> authenticate(result)
                else -> result.notImplemented()
            }
        }
    }

    override fun onStart() {
        super.onStart()
        if (Build.VERSION.SDK_INT >= 34 && capture == null) {
            capture = CaptureHooks(this).also { it.register() }
        }
    }

    override fun onStop() {
        if (Build.VERSION.SDK_INT >= 34) {
            capture?.unregister()
            capture = null
        }
        super.onStop()
    }

    private fun biometricsAvailable(): Boolean {
        if (Build.VERSION.SDK_INT < 29) return false
        return BiometricAvailability.enrolled(this)
    }

    private fun authenticate(result: MethodChannel.Result) {
        if (Build.VERSION.SDK_INT < 28) {
            result.success(false)
            return
        }
        BiometricGate.authenticate(this, result)
    }

    companion object {
        private const val KEYSTORE_CHANNEL = "io.github.femboynyx.umbra/keystore"
        private const val SCREEN_CHANNEL = "io.github.femboynyx.umbra/screen"
        private const val BIOMETRIC_CHANNEL = "io.github.femboynyx.umbra/biometric"
    }
}

/** Loaded only on API 34+, where [Activity.ScreenCaptureCallback] exists. */
@SuppressLint("NewApi")
private class CaptureHooks(private val activity: Activity) {
    private val callback =
        Activity.ScreenCaptureCallback {
            // A capture was observed where the OS did not block it.
            // That fact is not shown in the UI. Screen security is not
            // a guarantee, and a notice would not make it one.
        }

    fun register() {
        activity.registerScreenCaptureCallback(activity.mainExecutor, callback)
    }

    fun unregister() {
        activity.unregisterScreenCaptureCallback(callback)
    }
}

/**
 * Loaded only on API 29+, where [BiometricManager] exists.
 * The public SDK no longer exposes BiometricManager.from; the instance
 * comes from [Context.BIOMETRIC_SERVICE].
 */
@SuppressLint("NewApi")
private object BiometricAvailability {
    fun enrolled(activity: Activity): Boolean {
        val manager = activity.getSystemService(BiometricManager::class.java) ?: return false
        val code =
            if (Build.VERSION.SDK_INT >= 30) {
                manager.canAuthenticate(BiometricManager.Authenticators.BIOMETRIC_WEAK)
            } else {
                @Suppress("DEPRECATION")
                manager.canAuthenticate()
            }
        return code == BiometricManager.BIOMETRIC_SUCCESS
    }
}

/** Loaded only on API 28+, where [BiometricPrompt] exists. Soft lock only. */
@SuppressLint("NewApi")
private object BiometricGate {
    fun authenticate(activity: Activity, result: MethodChannel.Result) {
        val replied = AtomicBoolean(false)
        fun finish(ok: Boolean) {
            if (replied.compareAndSet(false, true)) result.success(ok)
        }
        val prompt =
            BiometricPrompt.Builder(activity)
                .setTitle("Unlock Umbra")
                .setSubtitle("Soft lock")
                .setNegativeButton("Cancel", activity.mainExecutor) { _, _ ->
                    finish(false)
                }
                .build()
        prompt.authenticate(
            CancellationSignal(),
            activity.mainExecutor,
            object : BiometricPrompt.AuthenticationCallback() {
                override fun onAuthenticationSucceeded(
                    authResult: BiometricPrompt.AuthenticationResult,
                ) {
                    finish(true)
                }

                override fun onAuthenticationError(errorCode: Int, errString: CharSequence) {
                    finish(false)
                }
            },
        )
    }
}

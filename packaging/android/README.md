# Android packaging for Umbra

App ID: `io.github.femboynyx.umbra`
Display name: `Umbra`

Copilot owns `app/`. Apply the snippets below there; do not commit secrets.

## Permissions (AndroidManifest.xml)

Add inside `<manifest>` (outside `<application>`):

```xml
<uses-permission android:name="android.permission.INTERNET"/>
<uses-permission android:name="android.permission.RECORD_AUDIO"/>
<uses-permission android:name="android.permission.CAMERA"/>
<uses-permission android:name="android.permission.MODIFY_AUDIO_SETTINGS"/>
<uses-permission android:name="android.permission.BLUETOOTH_CONNECT"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_MICROPHONE"/>
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_CAMERA"/>
<uses-permission android:name="android.permission.POST_NOTIFICATIONS"/>
```

`BLUETOOTH_CONNECT` and runtime mic/camera prompts are required for WebRTC calls on modern Android. `FOREGROUND_SERVICE*` keep calls alive when the app is backgrounded.

## Split-per-ABI APKs

In `app/android/app/build.gradle` or `build.gradle.kts`, prefer ABI splits (or App Bundle) so releases stay small:

```groovy
android {
    // Groovy
    splits {
        abi {
            enable true
            reset()
            include "armeabi-v7a", "arm64-v8a", "x86_64"
            universalApk true
        }
    }
}
```

```kotlin
// Kotlin DSL alternative: use bundle + Play, or
android {
    splits {
        abi {
            isEnable = true
            reset()
            include("armeabi-v7a", "arm64-v8a", "x86_64")
            isUniversalApk = true
        }
    }
}
```

CI may still upload a universal APK for GitHub Releases.

## Signing

See `key.properties.example` and `signing.gradle.snippet` / `signing.gradle.kts.snippet`.

Required GitHub Actions secrets (never commit these):

| Secret | Purpose |
|--------|---------|
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded `.jks` / `.keystore` |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Key alias |
| `ANDROID_KEY_PASSWORD` | Key password |

### Generate a release keystore (local)

```bash
keytool -genkey -v \
  -keystore umbra-release.jks \
  -keyalg RSA -keysize 2048 -validity 10000 \
  -alias umbra
base64 -w0 umbra-release.jks > umbra-release.jks.b64
# Store the .b64 contents in ANDROID_KEYSTORE_BASE64; delete local copies when done.
```

### CI decode pattern

```bash
echo "$ANDROID_KEYSTORE_BASE64" | base64 -d > "${RUNNER_TEMP}/umbra.jks"
cat > app/android/key.properties <<EOF
storePassword=$ANDROID_KEYSTORE_PASSWORD
keyPassword=$ANDROID_KEY_PASSWORD
keyAlias=$ANDROID_KEY_ALIAS
storeFile=${RUNNER_TEMP}/umbra.jks
EOF
```

If secrets are absent, fall back to debug signing and print a clear warning (release artifacts from that path must not be published as official builds).

`key.properties`, `*.jks`, and `*.keystore` are gitignored at the repo root.

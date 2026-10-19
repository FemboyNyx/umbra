// User-facing words for app lock and screen security.
//
// Screen security is never described as screenshot-proof. A sender's request
// for it is a request, not a guarantee (`docs/message-protection.md`).

const screenSecurityTitle = 'Screen security';

const screenSecurityBody =
    'Hides Umbra from casual screenshots, the app switcher, and screen sharing. '
    'It does not stop someone photographing the screen. A request for it is a request, not a guarantee.';

/// Shown when the OS cannot exclude the window. Linux is the case the doc
/// names; older Windows builds that reject display affinity use it too.
const linuxCaptureLimit =
    'Linux cannot block screenshots. Any X11 client can read the screen, and Wayland has no standard opt-out. Umbra covers the window when it loses focus, and that is all.';

const genericCaptureLimit =
    'Screenshots cannot be blocked on this device. Umbra covers the window when it loses focus, and that is all.';

const softLockTitle = 'Soft lock';

const softLockBody =
    'After Umbra has been in the background for 1 minute, ask for a PIN, passphrase, or biometrics before showing anything. Umbra keeps running. Biometrics never open a hard lock.';

const hardLockTitle = 'Hard lock';

const hardLockBody =
    'After 1 hour with no use, or when you lock now, Umbra stops the network. Nothing arrives until you enter the passphrase. Biometrics do not open a hard lock. Closing the database and wiping keys is not available from the app yet.';

/// The only text a locked Umbra shows about traffic. No sender and no body.
const newMessageWhileLocked = 'New message';

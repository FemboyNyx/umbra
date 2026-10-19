#ifndef UMBRA_KEYSTORE_H_
#define UMBRA_KEYSTORE_H_

#include <flutter_linux/flutter_linux.h>

// Registers the OS keystore channel. The 32-byte db_key is created once and
// stored in libsecret, or a mode-0600 file when libsecret is unavailable.
void umbra_register_keystore(FlView* view);

#endif  // UMBRA_KEYSTORE_H_

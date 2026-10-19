#ifndef UMBRA_SCREEN_H_
#define UMBRA_SCREEN_H_

#include <flutter_linux/flutter_linux.h>

// Linux cannot opt a window out of screen capture. The channel reports
// native=false so the app can say so and cover the window on focus loss.
void umbra_register_screen(FlView* view);

#endif  // UMBRA_SCREEN_H_

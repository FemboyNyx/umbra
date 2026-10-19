#include "umbra_screen.h"

#include <string.h>

namespace {

constexpr char kChannel[] = "io.github.femboynyx.umbra/screen";

void MethodCall(FlMethodChannel* channel, FlMethodCall* call, gpointer user_data) {
  (void)channel;
  (void)user_data;
  if (strcmp(fl_method_call_get_name(call), "setSecure") != 0) {
    g_autoptr(GError) error = nullptr;
    fl_method_call_respond_not_implemented(call, &error);
    return;
  }
  // No X11 or Wayland opt-out. The Flutter side covers the window.
  g_autoptr(FlValue) map = fl_value_new_map();
  fl_value_set_string_take(map, "native", fl_value_new_bool(false));
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond_success(call, map, &error)) {
    g_warning("Umbra screen response failed: %s", error->message);
  }
}

}  // namespace

void umbra_register_screen(FlView* view) {
  FlEngine* engine = fl_view_get_engine(view);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  // Intentionally leaked for the process lifetime; the channel must outlive
  // the registration call.
  FlMethodChannel* channel = fl_method_channel_new(
      fl_engine_get_binary_messenger(engine), kChannel, FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, MethodCall, nullptr, nullptr);
}

#include "screen_security.h"

#include <windows.h>

namespace {

constexpr char kChannel[] = "io.github.femboynyx.umbra/screen";

// Windows 10 2004+. The SDK this runner builds with may not define it.
#ifndef WDA_EXCLUDEFROMCAPTURE
#define WDA_EXCLUDEFROMCAPTURE 0x00000011
#endif

}  // namespace

UmbraScreenSecurity::UmbraScreenSecurity(flutter::BinaryMessenger* messenger,
                                         HWND hwnd)
    : hwnd_(hwnd),
      channel_(std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          messenger, kChannel,
          &flutter::StandardMethodCodec::GetInstance())) {
  channel_->SetMethodCallHandler(
      [this](const auto& call, auto result) { Handle(call, std::move(result)); });
}

void UmbraScreenSecurity::Handle(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  if (call.method_name() != "setSecure") {
    result->NotImplemented();
    return;
  }
  bool enabled = false;
  const auto* args = std::get_if<flutter::EncodableMap>(call.arguments());
  if (args != nullptr) {
    const auto found = args->find(flutter::EncodableValue("enabled"));
    if (found != args->end()) {
      if (const auto* flag = std::get_if<bool>(&found->second)) {
        enabled = *flag;
      }
    }
  }
  const DWORD affinity = enabled ? WDA_EXCLUDEFROMCAPTURE : WDA_NONE;
  const BOOL ok = SetWindowDisplayAffinity(hwnd_, affinity);
  flutter::EncodableMap map;
  map[flutter::EncodableValue("native")] =
      flutter::EncodableValue(ok == TRUE);
  result->Success(flutter::EncodableValue(map));
}

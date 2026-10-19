#ifndef RUNNER_SCREEN_SECURITY_H_
#define RUNNER_SCREEN_SECURITY_H_

#include <flutter/binary_messenger.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include <memory>

#include "win32_window.h"

// Excludes the window from screenshots, recordings, and screen sharing
// via SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE) on Windows 10 2004+.
class UmbraScreenSecurity {
 public:
  UmbraScreenSecurity(flutter::BinaryMessenger* messenger, HWND hwnd);

 private:
  void Handle(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  HWND hwnd_;
  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif  // RUNNER_SCREEN_SECURITY_H_

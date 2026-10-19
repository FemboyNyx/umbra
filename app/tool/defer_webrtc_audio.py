#!/usr/bin/env python3
"""Defer flutter_webrtc's ADM init until the first call, and fail soft.

The desktop plugin builds a PeerConnectionFactory in its constructor, which
runs at process start and aborts in adm_helpers.cc when no audio device
exists. Move that work to the first method call. If Pulse (Linux) or a wave
device (Windows) is missing, return an error instead of creating the factory.
"""

from __future__ import annotations

import sys
from pathlib import Path

MARKER = "UMBRA_DEFER_WEBRTC"

OLD_CTOR = """FlutterWebRTCBase::FlutterWebRTCBase(BinaryMessenger* messenger,
                                     TextureRegistrar* textures)
    : messenger_(messenger), textures_(textures) {
  LibWebRTC::Initialize();
  factory_ = LibWebRTC::CreateRTCPeerConnectionFactory();
  audio_device_ = factory_->GetAudioDevice();
  video_device_ = factory_->GetVideoDevice();
  desktop_device_ = factory_->GetDesktopDevice();
  event_channel_ = EventChannelProxy::Create(messenger_, kEventChannelName);
}

FlutterWebRTCBase::~FlutterWebRTCBase() {
  LibWebRTC::Terminate();
}
"""

NEW_CTOR = r"""FlutterWebRTCBase::FlutterWebRTCBase(BinaryMessenger* messenger,
                                     TextureRegistrar* textures)
    : messenger_(messenger), textures_(textures) {
  // UMBRA_DEFER_WEBRTC: do not touch the audio device at process start.
  event_channel_ = EventChannelProxy::Create(messenger_, kEventChannelName);
}

FlutterWebRTCBase::~FlutterWebRTCBase() {
  if (!webrtc_initialized_) return;
  factory_ = nullptr;
  audio_device_ = nullptr;
  video_device_ = nullptr;
  desktop_device_ = nullptr;
  LibWebRTC::Terminate();
}

namespace {

bool PathExists(const std::string& path) {
#if defined(_WIN32)
  const DWORD attr = GetFileAttributesA(path.c_str());
  return attr != INVALID_FILE_ATTRIBUTES;
#else
  return access(path.c_str(), F_OK) == 0;
#endif
}

// libwebrtc's desktop ADM aborts when Init fails. Probe first and skip the
// factory entirely when a call would hit that check.
bool DesktopAudioAvailable(std::string* error) {
#if defined(__linux__)
  if (const char* pulse = std::getenv("PULSE_SERVER")) {
    if (pulse[0] != '\0') return true;
  }
  if (const char* runtime = std::getenv("XDG_RUNTIME_DIR")) {
    if (runtime[0] != '\0' && PathExists(std::string(runtime) + "/pulse/native")) {
      return true;
    }
  }
  char user_pulse[64];
  std::snprintf(user_pulse, sizeof(user_pulse), "/run/user/%u/pulse/native",
                static_cast<unsigned>(getuid()));
  if (PathExists(user_pulse) || PathExists("/var/run/pulse/native")) return true;
  if (error) *error = "No audio device is available.";
  return false;
#elif defined(_WIN32)
  HMODULE winmm = LoadLibraryW(L"winmm.dll");
  if (!winmm) {
    if (error) *error = "No audio device is available.";
    return false;
  }
  using WaveOutGetNumDevs = UINT(WINAPI*)();
  auto fn = reinterpret_cast<WaveOutGetNumDevs>(
      GetProcAddress(winmm, "waveOutGetNumDevs"));
  const UINT devices = fn ? fn() : 0;
  FreeLibrary(winmm);
  if (devices == 0) {
    if (error) *error = "No audio device is available.";
    return false;
  }
  return true;
#else
  (void)error;
  return true;
#endif
}

}  // namespace

bool FlutterWebRTCBase::EnsureFactory(std::string* error) {
  if (factory_) return true;
  if (webrtc_init_failed_) {
    if (error) *error = webrtc_init_error_;
    return false;
  }
  std::string why;
  if (!DesktopAudioAvailable(&why)) {
    webrtc_init_failed_ = true;
    webrtc_init_error_ = why.empty() ? "No audio device is available." : why;
    if (error) *error = webrtc_init_error_;
    return false;
  }
  if (!LibWebRTC::Initialize()) {
    webrtc_init_failed_ = true;
    webrtc_init_error_ = "WebRTC failed to start.";
    if (error) *error = webrtc_init_error_;
    return false;
  }
  webrtc_initialized_ = true;
  factory_ = LibWebRTC::CreateRTCPeerConnectionFactory();
  if (!factory_) {
    webrtc_init_failed_ = true;
    webrtc_init_error_ = "WebRTC failed to start.";
    if (error) *error = webrtc_init_error_;
    return false;
  }
  audio_device_ = factory_->GetAudioDevice();
  video_device_ = factory_->GetVideoDevice();
  desktop_device_ = factory_->GetDesktopDevice();
  return true;
}
"""

OLD_HANDLE = """void FlutterWebRTC::HandleMethodCall(
    const MethodCallProxy& method_call,
    std::unique_ptr<MethodResultProxy> result) {
  if (method_call.method_name().compare("initialize") == 0) {
"""

NEW_HANDLE = """void FlutterWebRTC::HandleMethodCall(
    const MethodCallProxy& method_call,
    std::unique_ptr<MethodResultProxy> result) {
  // UMBRA_DEFER_WEBRTC: ADM init happens here, on the first call, not at launch.
  std::string audio_error;
  if (!EnsureFactory(&audio_error)) {
    result->Error("no_audio_device", audio_error);
    return;
  }
  if (method_call.method_name().compare("initialize") == 0) {
"""

OLD_DECL = """  FlutterWebRTCBase(BinaryMessenger* messenger, TextureRegistrar* textures);
  ~FlutterWebRTCBase();
"""

NEW_DECL = """  FlutterWebRTCBase(BinaryMessenger* messenger, TextureRegistrar* textures);
  ~FlutterWebRTCBase();

  // UMBRA_DEFER_WEBRTC
  bool EnsureFactory(std::string* error);
"""

OLD_FIELDS = """  scoped_refptr<RTCPeerConnectionFactory> factory_;
"""

NEW_FIELDS = """  scoped_refptr<RTCPeerConnectionFactory> factory_;
  bool webrtc_initialized_ = false;
  bool webrtc_init_failed_ = false;
  std::string webrtc_init_error_;
"""

INCLUDES = """#include "flutter_webrtc_base.h"

#include "flutter_data_channel.h"
#include "flutter_peerconnection.h"
"""

INCLUDES_NEW = """#include "flutter_webrtc_base.h"

#include <cstdio>
#include <cstdlib>
#include <string>

#include "flutter_data_channel.h"
#include "flutter_peerconnection.h"

#if defined(__linux__)
#include <unistd.h>
#endif
#if defined(_WIN32)
#include <windows.h>
#endif
"""


def _replace(path: Path, old: str, new: str, label: str) -> None:
    text = path.read_text()
    if old not in text:
        raise SystemExit(f"{path}: expected {label} snippet was not found")
    path.write_text(text.replace(old, new, 1))


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} <flutter_webrtc package root>")
    root = Path(sys.argv[1])
    base_cc = root / "common/cpp/src/flutter_webrtc_base.cc"
    base_h = root / "common/cpp/include/flutter_webrtc_base.h"
    api_cc = root / "common/cpp/src/flutter_webrtc.cc"
    if not base_cc.is_file():
        raise SystemExit(f"missing {base_cc}")
    if MARKER in base_cc.read_text():
        return
    _replace(base_cc, INCLUDES, INCLUDES_NEW, "includes")
    _replace(base_cc, OLD_CTOR, NEW_CTOR, "constructor")
    _replace(base_h, OLD_DECL, NEW_DECL, "declaration")
    _replace(base_h, OLD_FIELDS, NEW_FIELDS, "fields")
    _replace(api_cc, OLD_HANDLE, NEW_HANDLE, "HandleMethodCall")


if __name__ == "__main__":
    main()

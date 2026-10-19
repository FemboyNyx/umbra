#include "keystore.h"

#include <windows.h>
#include <wincrypt.h>
#include <bcrypt.h>

#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace {

constexpr char kChannel[] = "io.github.femboynyx.umbra/keystore";

std::wstring Utf8ToWide(const std::string& utf8) {
  if (utf8.empty()) return L"";
  const int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS,
                                        utf8.data(), static_cast<int>(utf8.size()),
                                        nullptr, 0);
  if (size <= 0) return L"";
  std::wstring out(size, L'\0');
  MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, utf8.data(),
                      static_cast<int>(utf8.size()), out.data(), size);
  return out;
}

bool Random32(uint8_t out[32]) {
  return BCryptGenRandom(nullptr, out, 32, BCRYPT_USE_SYSTEM_PREFERRED_RNG) ==
         0;
}

bool Protect(const uint8_t key[32], std::vector<uint8_t>* blob) {
  DATA_BLOB input;
  input.pbData = const_cast<BYTE*>(key);
  input.cbData = 32;
  DATA_BLOB output{};
  if (!CryptProtectData(&input, L"Umbra db_key", nullptr, nullptr, nullptr,
                        CRYPTPROTECT_UI_FORBIDDEN, &output)) {
    return false;
  }
  blob->assign(output.pbData, output.pbData + output.cbData);
  LocalFree(output.pbData);
  return true;
}

bool Unprotect(const std::vector<uint8_t>& blob, uint8_t key[32]) {
  if (blob.empty()) return false;
  DATA_BLOB input;
  input.pbData = const_cast<BYTE*>(blob.data());
  input.cbData = static_cast<DWORD>(blob.size());
  DATA_BLOB output{};
  if (!CryptUnprotectData(&input, nullptr, nullptr, nullptr, nullptr,
                          CRYPTPROTECT_UI_FORBIDDEN, &output)) {
    return false;
  }
  const bool ok = output.cbData == 32;
  if (ok) memcpy(key, output.pbData, 32);
  SecureZeroMemory(output.pbData, output.cbData);
  LocalFree(output.pbData);
  return ok;
}

bool ReadFileBytes(const std::filesystem::path& path, std::vector<uint8_t>* out) {
  std::ifstream in(path, std::ios::binary);
  if (!in) return false;
  out->assign(std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>());
  return in.good() || in.eof();
}

bool WriteFileBytes(const std::filesystem::path& path, const std::vector<uint8_t>& blob) {
  std::ofstream out(path, std::ios::binary | std::ios::trunc);
  if (!out) return false;
  out.write(reinterpret_cast<const char*>(blob.data()),
            static_cast<std::streamsize>(blob.size()));
  return static_cast<bool>(out);
}

void Succeed(std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result,
             uint8_t key[32]) {
  flutter::EncodableMap map;
  map[flutter::EncodableValue("bytes")] =
      flutter::EncodableValue(std::vector<uint8_t>(key, key + 32));
  map[flutter::EncodableValue("backend")] = flutter::EncodableValue("dpapi");
  SecureZeroMemory(key, 32);
  result->Success(flutter::EncodableValue(map));
}

}  // namespace

UmbraKeystore::UmbraKeystore(flutter::BinaryMessenger* messenger)
    : channel_(std::make_unique<flutter::MethodChannel<flutter::EncodableValue>>(
          messenger, kChannel, &flutter::StandardMethodCodec::GetInstance())) {
  channel_->SetMethodCallHandler(
      [this](const auto& call, auto result) { Handle(call, std::move(result)); });
}

void UmbraKeystore::Handle(
    const flutter::MethodCall<flutter::EncodableValue>& call,
    std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result) {
  if (call.method_name() != "getOrCreateDbKey") {
    result->NotImplemented();
    return;
  }
  const auto* args = std::get_if<flutter::EncodableMap>(call.arguments());
  if (args == nullptr) {
    result->Error("keystore", "storageDir is required");
    return;
  }
  const auto found = args->find(flutter::EncodableValue("storageDir"));
  if (found == args->end()) {
    result->Error("keystore", "storageDir is required");
    return;
  }
  const auto* dir = std::get_if<std::string>(&found->second);
  if (dir == nullptr || dir->empty()) {
    result->Error("keystore", "storageDir is required");
    return;
  }
  const std::wstring wide = Utf8ToWide(*dir);
  if (wide.empty()) {
    result->Error("keystore", "storageDir is not valid UTF-8");
    return;
  }
  std::error_code ec;
  std::filesystem::create_directories(wide, ec);
  // DPAPI blob for this Windows user only. There is no allowBackup
  // equivalent; a copy of the support folder takes umbra.db, epoch, and this
  // blob together, and the blob does not unwrap for another user.
  const auto path = std::filesystem::path(wide) / L"db_key.dpapi";

  uint8_t key[32];
  std::vector<uint8_t> existing;
  if (std::filesystem::exists(path)) {
    if (!ReadFileBytes(path, &existing) || !Unprotect(existing, key)) {
      SecureZeroMemory(existing.data(), existing.size());
      result->Error("keystore", "stored db_key could not be unwrapped");
      return;
    }
    SecureZeroMemory(existing.data(), existing.size());
    Succeed(std::move(result), key);
    return;
  }

  if (!Random32(key)) {
    result->Error("keystore", "could not generate db_key");
    return;
  }
  std::vector<uint8_t> blob;
  if (!Protect(key, &blob) || !WriteFileBytes(path, blob)) {
    SecureZeroMemory(key, sizeof(key));
    SecureZeroMemory(blob.data(), blob.size());
    result->Error("keystore", "could not store db_key");
    return;
  }
  SecureZeroMemory(blob.data(), blob.size());
  Succeed(std::move(result), key);
}

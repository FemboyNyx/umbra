#include "umbra_keystore.h"

#include <dlfcn.h>
#include <errno.h>
#include <fcntl.h>
#include <string.h>
#include <sys/random.h>
#include <sys/stat.h>
#include <unistd.h>

namespace {

constexpr char kChannel[] = "io.github.femboynyx.umbra/keystore";
constexpr char kSchemaName[] = "io.github.femboynyx.umbra.DbKey";

struct SecretSchemaAttribute {
  const char* name;
  int type;
};

struct SecretSchema {
  const char* name;
  int flags;
  SecretSchemaAttribute attributes[32];
  int reserved;
  void* reserved1;
  void* reserved2;
  void* reserved3;
  void* reserved4;
  void* reserved5;
  void* reserved6;
  void* reserved7;
};

using StoreFn = int (*)(const void*,
                        const char*,
                        const char*,
                        const char*,
                        void*,
                        void**,
                        ...);
using LookupFn = char* (*)(const void*, void*, void**, ...);
using FreeFn = void (*)(char*);

struct SecretApi {
  void* library = nullptr;
  StoreFn store = nullptr;
  LookupFn lookup = nullptr;
  FreeFn free_password = nullptr;
};

SecretApi LoadSecret() {
  SecretApi api;
  api.library = dlopen("libsecret-1.so.0", RTLD_LAZY);
  if (api.library == nullptr) return api;
  api.store = reinterpret_cast<StoreFn>(dlsym(api.library, "secret_password_store_sync"));
  api.lookup = reinterpret_cast<LookupFn>(dlsym(api.library, "secret_password_lookup_sync"));
  api.free_password = reinterpret_cast<FreeFn>(dlsym(api.library, "secret_password_free"));
  if (api.store == nullptr || api.lookup == nullptr || api.free_password == nullptr) {
    dlclose(api.library);
    api = SecretApi{};
  }
  return api;
}

const SecretSchema& Schema() {
  static const SecretSchema schema = {
      kSchemaName,
      0,
      {{"service", 0}, {"account", 0}, {nullptr, 0}},
      0,
      nullptr,
      nullptr,
      nullptr,
      nullptr,
      nullptr,
      nullptr,
      nullptr,
  };
  return schema;
}

bool RandomBytes(uint8_t* out) {
  size_t filled = 0;
  while (filled < 32) {
    const ssize_t n = getrandom(out + filled, 32 - filled, 0);
    if (n < 0) {
      if (errno == EINTR) continue;
      return false;
    }
    filled += static_cast<size_t>(n);
  }
  return true;
}

gchar* FilePath(const gchar* dir) {
  // libsecret is preferred. This file is the fallback secret and sits in the
  // data directory next to umbra.db and epoch. Linux has no allowBackup API;
  // the file is mode 0600 and is not an OS keystore item.
  return g_build_filename(dir, "db_key.bin", nullptr);
}

bool ReadFile(const gchar* path, uint8_t* out) {
  const int fd = open(path, O_RDONLY);
  if (fd < 0) return false;
  size_t filled = 0;
  while (filled < 32) {
    const ssize_t n = read(fd, out + filled, 32 - filled);
    if (n < 0) {
      if (errno == EINTR) continue;
      close(fd);
      return false;
    }
    if (n == 0) break;
    filled += static_cast<size_t>(n);
  }
  uint8_t extra = 0;
  const ssize_t more = read(fd, &extra, 1);
  close(fd);
  return filled == 32 && more == 0;
}

bool WriteFile(const gchar* path, const uint8_t* bytes) {
  const int fd = open(path, O_WRONLY | O_CREAT | O_TRUNC, 0600);
  if (fd < 0) return false;
  size_t written = 0;
  while (written < 32) {
    const ssize_t n = write(fd, bytes + written, 32 - written);
    if (n < 0) {
      if (errno == EINTR) continue;
      close(fd);
      return false;
    }
    written += static_cast<size_t>(n);
  }
  fchmod(fd, 0600);
  close(fd);
  return true;
}

bool SecretStore(SecretApi api, const gchar* b64) {
  return api.store(&Schema(), "default", "Umbra database key", b64, nullptr, nullptr,
                   "service", "io.github.femboynyx.umbra", "account", "db_key",
                   nullptr) != 0;
}

gchar* SecretLookup(SecretApi api) {
  return api.lookup(&Schema(), nullptr, nullptr, "service",
                    "io.github.femboynyx.umbra", "account", "db_key", nullptr);
}

bool DecodeKey(const gchar* b64, uint8_t* out) {
  gsize len = 0;
  guchar* raw = g_base64_decode(b64, &len);
  if (raw == nullptr || len != 32) {
    g_free(raw);
    return false;
  }
  memcpy(out, raw, 32);
  g_free(raw);
  return true;
}

void Respond(FlMethodCall* call, const uint8_t* key, const char* backend) {
  g_autoptr(FlValue) map = fl_value_new_map();
  fl_value_set_string_take(map, "bytes", fl_value_new_uint8_list(key, 32));
  fl_value_set_string_take(map, "backend", fl_value_new_string(backend));
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond_success(call, map, &error)) {
    g_warning("Umbra keystore response failed: %s", error->message);
  }
}

void RespondError(FlMethodCall* call, const char* message) {
  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond_error(call, "keystore", message, nullptr, &error)) {
    g_warning("Umbra keystore error response failed: %s", error->message);
  }
}

void Handle(FlMethodCall* call) {
  FlValue* args = fl_method_call_get_args(call);
  FlValue* dir_value = fl_value_lookup_string(args, "storageDir");
  if (dir_value == nullptr || fl_value_get_type(dir_value) != FL_VALUE_TYPE_STRING) {
    RespondError(call, "storageDir is required");
    return;
  }
  const gchar* dir = fl_value_get_string(dir_value);
  g_mkdir_with_parents(dir, 0700);

  uint8_t key[32];
  const SecretApi api = LoadSecret();
  g_autofree gchar* path = FilePath(dir);

  if (api.library != nullptr) {
    gchar* existing = SecretLookup(api);
    if (existing != nullptr) {
      const bool ok = DecodeKey(existing, key);
      api.free_password(existing);
      dlclose(api.library);
      if (!ok) {
        RespondError(call, "stored db_key is not 32 bytes");
        return;
      }
      Respond(call, key, "libsecret");
      explicit_bzero(key, sizeof(key));
      return;
    }
    if (ReadFile(path, key)) {
      gchar* b64 = g_base64_encode(key, 32);
      const bool stored = SecretStore(api, b64);
      g_free(b64);
      dlclose(api.library);
      if (!stored) {
        Respond(call, key, "file");
        explicit_bzero(key, sizeof(key));
        return;
      }
      unlink(path);
      Respond(call, key, "libsecret");
      explicit_bzero(key, sizeof(key));
      return;
    }
  }

  if (ReadFile(path, key)) {
    if (api.library != nullptr) dlclose(api.library);
    Respond(call, key, "file");
    explicit_bzero(key, sizeof(key));
    return;
  }

  if (!RandomBytes(key) || !WriteFile(path, key)) {
    if (api.library != nullptr) dlclose(api.library);
    RespondError(call, "could not create db_key");
    return;
  }
  if (api.library != nullptr) {
    gchar* b64 = g_base64_encode(key, 32);
    const bool stored = SecretStore(api, b64);
    g_free(b64);
    dlclose(api.library);
    if (stored) {
      unlink(path);
      Respond(call, key, "libsecret");
      explicit_bzero(key, sizeof(key));
      return;
    }
  }
  Respond(call, key, "file");
  explicit_bzero(key, sizeof(key));
}

void MethodCall(FlMethodChannel* channel, FlMethodCall* call, gpointer user_data) {
  (void)channel;
  (void)user_data;
  if (strcmp(fl_method_call_get_name(call), "getOrCreateDbKey") != 0) {
    g_autoptr(GError) error = nullptr;
    fl_method_call_respond_not_implemented(call, &error);
    return;
  }
  Handle(call);
}

}  // namespace

void umbra_register_keystore(FlView* view) {
  FlEngine* engine = fl_view_get_engine(view);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  // Intentionally leaked for the process lifetime; the channel must outlive
  // the registration call.
  FlMethodChannel* channel = fl_method_channel_new(
      fl_engine_get_binary_messenger(engine), kChannel, FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(channel, MethodCall, nullptr, nullptr);
}

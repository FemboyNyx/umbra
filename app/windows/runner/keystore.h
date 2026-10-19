#ifndef RUNNER_KEYSTORE_H_
#define RUNNER_KEYSTORE_H_

#include <flutter/binary_messenger.h>
#include <flutter/method_channel.h>
#include <flutter/standard_method_codec.h>

#include <memory>

// Holds the SQLCipher key in a DPAPI blob at {storageDir}/db_key.dpapi.
class UmbraKeystore {
 public:
  explicit UmbraKeystore(flutter::BinaryMessenger* messenger);

 private:
  void Handle(
      const flutter::MethodCall<flutter::EncodableValue>& call,
      std::unique_ptr<flutter::MethodResult<flutter::EncodableValue>> result);

  std::unique_ptr<flutter::MethodChannel<flutter::EncodableValue>> channel_;
};

#endif  // RUNNER_KEYSTORE_H_

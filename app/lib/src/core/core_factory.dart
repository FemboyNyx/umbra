import 'frb_umbra_core.dart';
import 'mock_umbra_core.dart';
import 'umbra_core.dart';

/// [mock] defaults to true. Real mode uses the generated bridge.
UmbraCore createUmbraCore({
  required bool mock,
  MockTiming timing = const MockTiming(),
}) {
  if (mock) return MockUmbraCore(timing: timing);
  return FrbUmbraCore();
}

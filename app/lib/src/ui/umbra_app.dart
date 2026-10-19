import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../lock/app_lock.dart';
import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';
import 'contacts_screen.dart';
import 'lock_screen.dart';
import 'onboarding_screen.dart';
import 'umbra_mark.dart';

class UmbraApp extends StatefulWidget {
  const UmbraApp({super.key, required this.controller, this.autoStart = true});

  final UmbraController controller;
  final bool autoStart;

  @override
  State<UmbraApp> createState() => _UmbraAppState();
}

class _UmbraAppState extends State<UmbraApp> with WidgetsBindingObserver {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    HardwareKeyboard.instance.addHandler(_onKey);
    widget.controller.addListener(_onChange);
    widget.controller.appLock.addListener(_onChange);
    WidgetsBinding.instance.addPostFrameCallback((_) {
      widget.controller.appLock.applyScreenSecurity();
      if (widget.autoStart && !widget.controller.started) {
        widget.controller.bootstrap();
      }
    });
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    widget.controller.appLock.noteLifecycle(state);
  }

  bool _onKey(KeyEvent event) {
    widget.controller.appLock.noteActivity();
    return false;
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    WidgetsBinding.instance.removeObserver(this);
    widget.controller.appLock.removeListener(_onChange);
    widget.controller.removeListener(_onChange);
    super.dispose();
  }

  void _onChange() {
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    return MaterialApp(
      title: 'Umbra',
      debugShowCheckedModeBanner: false,
      theme: UmbraTheme.light(),
      darkTheme: UmbraTheme.dark(),
      themeMode: controller.settings.themeMode,
      builder: (context, child) {
        final lock = controller.appLock;
        final covered = lock.locked || lock.obscured;
        return Listener(
          onPointerDown: (_) => lock.noteActivity(),
          child: Stack(
            fit: StackFit.expand,
            children: [
              IgnorePointer(
                ignoring: covered,
                child: child ?? const SizedBox.shrink(),
              ),
              if (lock.locked) _LockLayer(lock: lock),
              if (lock.obscured) const PrivacyCover(),
            ],
          ),
        );
      },
      home: _Gate(controller: controller),
    );
  }
}

class _Gate extends StatelessWidget {
  const _Gate({required this.controller});

  final UmbraController controller;

  @override
  Widget build(BuildContext context) {
    if (!controller.started && controller.error != null) {
      return _ErrorScreen(controller: controller);
    }
    if (!controller.started) {
      return const _Splash();
    }
    if (controller.needsOnboarding) {
      return OnboardingScreen(controller: controller);
    }
    return ContactsScreen(controller: controller);
  }
}

/// The lock sits above every route, so it needs its own overlay for the
/// passphrase field.
class _LockLayer extends StatefulWidget {
  const _LockLayer({required this.lock});

  final AppLock lock;

  @override
  State<_LockLayer> createState() => _LockLayerState();
}

class _LockLayerState extends State<_LockLayer> {
  late final OverlayEntry _entry = OverlayEntry(
    builder: (context) =>
        LockScreen(key: const Key('app-lock'), lock: widget.lock),
  );

  @override
  void didUpdateWidget(covariant _LockLayer oldWidget) {
    super.didUpdateWidget(oldWidget);
    _entry.markNeedsBuild();
  }

  @override
  Widget build(BuildContext context) {
    return Overlay(initialEntries: [_entry]);
  }
}

class _Splash extends StatelessWidget {
  const _Splash();

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      body: Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            UmbraMark(size: 88),
            SizedBox(height: 20),
            Text('Umbra'),
            SizedBox(height: 16),
            CircularProgressIndicator(),
          ],
        ),
      ),
    );
  }
}

class _ErrorScreen extends StatelessWidget {
  const _ErrorScreen({required this.controller});

  final UmbraController controller;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(28),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Spacer(),
              Text(
                'Umbra could not start',
                style: Theme.of(context).textTheme.headlineSmall,
              ),
              const SizedBox(height: 12),
              Text(controller.error ?? ''),
              const SizedBox(height: 24),
              FilledButton(
                onPressed: controller.booting ? null : controller.bootstrap,
                child: const Text('Try again'),
              ),
              const SizedBox(height: 8),
              if (!controller.settings.mock)
                OutlinedButton(
                  onPressed: controller.booting
                      ? null
                      : controller.fallBackToMock,
                  child: const Text('Use the mock backend'),
                ),
              const Spacer(),
            ],
          ),
        ),
      ),
    );
  }
}

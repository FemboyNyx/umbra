import 'package:flutter/material.dart';

import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';
import 'contacts_screen.dart';
import 'onboarding_screen.dart';
import 'umbra_mark.dart';

class UmbraApp extends StatefulWidget {
  const UmbraApp({super.key, required this.controller, this.autoStart = true});

  final UmbraController controller;
  final bool autoStart;

  @override
  State<UmbraApp> createState() => _UmbraAppState();
}

class _UmbraAppState extends State<UmbraApp> {
  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onChange);
    if (widget.autoStart && !widget.controller.started) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        widget.controller.bootstrap();
      });
    }
  }

  @override
  void dispose() {
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

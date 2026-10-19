import 'package:flutter/material.dart';

import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';
import 'umbra_mark.dart';

class OnboardingScreen extends StatefulWidget {
  const OnboardingScreen({super.key, required this.controller});

  final UmbraController controller;

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen> {
  final _name = TextEditingController();
  String? _error;
  var _saving = false;

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  Future<void> _continue() async {
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await widget.controller.setDisplayName(_name.text);
    } catch (err) {
      setState(() => _error = err.toString());
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 32),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 440),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  const Center(child: BrandLockup(size: 128)),
                  const SizedBox(height: 28),
                  Text(
                    'Umbra',
                    textAlign: TextAlign.center,
                    style: theme.textTheme.headlineLarge?.copyWith(
                      fontWeight: FontWeight.w600,
                      letterSpacing: 0.4,
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'Talk in the shadows.',
                    textAlign: TextAlign.center,
                    style: theme.textTheme.titleMedium?.copyWith(
                      color: UmbraColors.mist,
                    ),
                  ),
                  const SizedBox(height: 20),
                  Text(
                    'There is no Umbra account. You pick a display name for this device, and people add you by comparing a safety number. Messages stay encrypted on your devices.',
                    style: theme.textTheme.bodyMedium?.copyWith(height: 1.45),
                  ),
                  const SizedBox(height: 28),
                  TextField(
                    controller: _name,
                    textInputAction: TextInputAction.done,
                    textCapitalization: TextCapitalization.words,
                    autofocus: true,
                    onSubmitted: (_) => _continue(),
                    decoration: const InputDecoration(
                      labelText: 'Display name',
                      hintText: 'What should contacts call you?',
                    ),
                  ),
                  if (_error != null) ...[
                    const SizedBox(height: 12),
                    Text(
                      _error!,
                      style: TextStyle(color: theme.colorScheme.error),
                    ),
                  ],
                  const SizedBox(height: 20),
                  FilledButton(
                    onPressed: _saving ? null : _continue,
                    child: Text(_saving ? 'Saving…' : 'Continue'),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

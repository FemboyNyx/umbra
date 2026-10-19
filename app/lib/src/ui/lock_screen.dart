import 'package:flutter/material.dart';

import '../lock/app_lock.dart';
import '../theme/umbra_theme.dart';
import 'umbra_mark.dart';

/// Full-screen gate. Shows no chats, senders, or message text.
class LockScreen extends StatefulWidget {
  const LockScreen({super.key, required this.lock});

  final AppLock lock;

  @override
  State<LockScreen> createState() => _LockScreenState();
}

class _LockScreenState extends State<LockScreen> {
  final _passphrase = TextEditingController();
  var _busy = false;
  var _biometric = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    if (!widget.lock.hardLocked) {
      widget.lock.biometric.available().then((ok) {
        if (mounted) setState(() => _biometric = ok);
      });
    }
  }

  @override
  void dispose() {
    _passphrase.clear();
    _passphrase.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final hard = widget.lock.hardLocked;
    final theme = Theme.of(context);
    return Material(
      color: theme.colorScheme.surface,
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.all(28),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              const Spacer(),
              const Center(child: UmbraMark(size: 72)),
              const SizedBox(height: 20),
              Text(
                'Umbra is locked',
                style: theme.textTheme.headlineSmall,
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 8),
              Text(
                hard
                    ? 'Enter the passphrase. Biometrics do not open this.'
                    : 'Enter the PIN or passphrase.',
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: UmbraColors.mist,
                ),
                textAlign: TextAlign.center,
              ),
              if (widget.lock.lockedNotice != null) ...[
                const SizedBox(height: 16),
                Text(
                  widget.lock.lockedNotice!,
                  textAlign: TextAlign.center,
                  style: theme.textTheme.titleMedium,
                ),
              ],
              const SizedBox(height: 24),
              TextField(
                controller: _passphrase,
                obscureText: true,
                autofocus: true,
                decoration: InputDecoration(
                  labelText: hard ? 'Passphrase' : 'PIN or passphrase',
                  errorText: _error,
                ),
                onSubmitted: (_) => _unlock(biometric: false),
              ),
              const SizedBox(height: 12),
              FilledButton(
                onPressed: _busy ? null : () => _unlock(biometric: false),
                child: const Text('Unlock'),
              ),
              if (!hard && _biometric) ...[
                const SizedBox(height: 8),
                OutlinedButton(
                  onPressed: _busy ? null : () => _unlock(biometric: true),
                  child: const Text('Use biometrics'),
                ),
              ],
              const Spacer(),
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _unlock({required bool biometric}) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    final error = await widget.lock.unlock(
      _passphrase.text,
      biometric: biometric,
    );
    _passphrase.clear();
    if (!mounted) return;
    setState(() {
      _busy = false;
      _error = error;
    });
  }
}

/// Opaque cover for a window that lost focus and cannot opt out of capture.
class PrivacyCover extends StatelessWidget {
  const PrivacyCover({super.key});

  @override
  Widget build(BuildContext context) {
    return const Material(
      key: Key('privacy-cover'),
      child: Center(child: UmbraMark(size: 72)),
    );
  }
}

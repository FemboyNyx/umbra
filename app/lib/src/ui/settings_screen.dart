import 'package:flutter/material.dart';
import 'package:umbra_calls/umbra_calls.dart' show torRelayRequiredMessage;

import '../lock/lock_copy.dart';
import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key, required this.controller});

  final UmbraController controller;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  late final TextEditingController _name;
  late final TextEditingController _tor;

  @override
  void initState() {
    super.initState();
    _name = TextEditingController(text: widget.controller.self?.displayName);
    _tor = TextEditingController(text: widget.controller.settings.torSocks);
  }

  @override
  void dispose() {
    _name.dispose();
    _tor.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Listenable.merge([
        widget.controller,
        widget.controller.appLock,
      ]),
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final controller = widget.controller;
    final theme = Theme.of(context);
    final self = controller.self;
    final network = controller.network;
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        key: const Key('settings-list'),
        padding: const EdgeInsets.fromLTRB(20, 12, 20, 32),
        children: [
          TextField(
            controller: _name,
            decoration: const InputDecoration(labelText: 'Display name'),
            onSubmitted: (value) => _saveName(value),
          ),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton(
              onPressed: () => _saveName(_name.text),
              child: const Text('Save name'),
            ),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Mock backend'),
            subtitle: const Text(
              'Fake contacts and echo replies, for UI work. Turning this off uses the Rust core on the next launch.',
            ),
            value: controller.settings.mock,
            onChanged: (value) async {
              await controller.updateSettings(mock: value);
              if (!context.mounted) return;
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text('Restart Umbra to apply the backend change.'),
                ),
              );
            },
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('LAN discovery (mDNS)'),
            subtitle: const Text(
              'Off by default, and always off when a Tor SOCKS proxy is set. Applies on the next launch.',
            ),
            value: controller.settings.enableMdns,
            onChanged: (value) => controller.updateSettings(enableMdns: value),
          ),
          TextField(
            controller: _tor,
            decoration: const InputDecoration(
              labelText: 'Tor SOCKS address',
              hintText: '127.0.0.1:9050',
              helperText: 'Empty means clearnet. Applies on the next launch.',
            ),
            onSubmitted: (value) => controller.updateSettings(torSocks: value),
          ),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerRight,
            child: TextButton(
              onPressed: () => controller.updateSettings(torSocks: _tor.text),
              child: const Text('Save Tor address'),
            ),
          ),
          const SizedBox(height: 8),
          Text('Lock', style: theme.textTheme.titleMedium),
          const SizedBox(height: 4),
          SwitchListTile(
            key: const Key('soft-lock'),
            contentPadding: EdgeInsets.zero,
            title: const Text(softLockTitle),
            subtitle: const Text(softLockBody),
            value: controller.appLock.settings.softLock,
            onChanged: (value) => _setSoftLock(value),
          ),
          SwitchListTile(
            key: const Key('hard-lock'),
            contentPadding: EdgeInsets.zero,
            title: const Text(hardLockTitle),
            subtitle: const Text(hardLockBody),
            value: controller.appLock.settings.hardLock,
            onChanged: (value) => _setHardLock(value),
          ),
          if (controller.appLock.settings.softLock ||
              controller.appLock.settings.hardLock)
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: () => controller.appLock.lockNow(),
                child: const Text('Lock now'),
              ),
            ),
          if (controller.appLock.lastError != null)
            Text(
              controller.appLock.lastError!,
              style: theme.textTheme.bodySmall?.copyWith(
                color: UmbraColors.danger,
              ),
            ),
          const SizedBox(height: 8),
          Text(screenSecurityTitle, style: theme.textTheme.titleMedium),
          const SizedBox(height: 4),
          SwitchListTile(
            key: const Key('screen-security'),
            contentPadding: EdgeInsets.zero,
            title: const Text(screenSecurityTitle),
            subtitle: Text(
              [
                screenSecurityBody,
                if (controller.appLock.captureLimitText != null)
                  controller.appLock.captureLimitText!,
              ].join(' '),
            ),
            value: controller.appLock.settings.screenSecurity,
            onChanged: (value) => controller.appLock.setScreenSecurity(value),
          ),
          const SizedBox(height: 8),
          Text('Appearance', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          SegmentedButton<ThemeMode>(
            segments: const [
              ButtonSegment(value: ThemeMode.dark, label: Text('Dark')),
              ButtonSegment(value: ThemeMode.light, label: Text('Light')),
              ButtonSegment(value: ThemeMode.system, label: Text('System')),
            ],
            selected: {controller.settings.themeMode},
            onSelectionChanged: (values) {
              controller.updateSettings(themeMode: values.first);
            },
          ),
          const SizedBox(height: 24),
          Text('This device', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          _InfoRow(label: 'Peer id', value: self?.peerId ?? '—'),
          _InfoRow(label: 'Fingerprint', value: self?.fingerprint ?? '—'),
          _InfoRow(
            label: 'Key store',
            value: controller.keystoreBackend.isEmpty
                ? '—'
                : controller.keystoreBackend,
          ),
          _InfoRow(
            label: 'Network',
            value: network.running
                ? 'Running · ${network.connectedPeers} ${network.connectedPeers == 1 ? 'peer' : 'peers'}'
                : 'Stopped',
          ),
          if (network.listenAddrs.isNotEmpty)
            _InfoRow(label: 'Listening', value: network.listenAddrs.join('\n')),
          const SizedBox(height: 24),
          Text('About', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(
            'Umbra is peer-to-peer and end-to-end encrypted. There is no Umbra server. This build has not had a security audit — do not rely on it for high-risk use.',
            style: theme.textTheme.bodyMedium?.copyWith(height: 1.4),
          ),
          const SizedBox(height: 8),
          Text(
            'In Tor mode, calls are relay-only and stay blocked until a TURN server is configured.',
            style: theme.textTheme.bodyMedium?.copyWith(height: 1.4),
          ),
          if (controller.callsBlockedByTor) ...[
            const SizedBox(height: 8),
            Text(
              torRelayRequiredMessage,
              style: theme.textTheme.bodyMedium?.copyWith(
                color: UmbraColors.danger,
                height: 1.4,
              ),
            ),
          ],
          const SizedBox(height: 8),
          Text(
            'License: AGPL-3.0-or-later. Private keys stay in the Rust core. This screen only shows decrypted text and public identity material.',
            style: theme.textTheme.bodySmall?.copyWith(
              color: UmbraColors.mist,
              height: 1.4,
            ),
          ),
        ],
      ),
    );
  }

  Future<void> _setSoftLock(bool enabled) async {
    final lock = widget.controller.appLock;
    if (enabled && !lock.hasPassphrase) {
      final chosen = await _askNewPassphrase();
      if (chosen == null || !mounted) return;
      await lock.setPassphrase(chosen);
    }
    if (!enabled && lock.hasPassphrase) {
      final ok = await _confirmPassphrase();
      if (ok != true || !mounted) return;
    }
    await lock.setSoftLock(enabled);
  }

  Future<void> _setHardLock(bool enabled) async {
    final lock = widget.controller.appLock;
    if (enabled && !lock.hasPassphrase) {
      final chosen = await _askNewPassphrase();
      if (chosen == null || !mounted) return;
      await lock.setPassphrase(chosen);
    }
    if (!enabled && lock.hasPassphrase) {
      final ok = await _confirmPassphrase();
      if (ok != true || !mounted) return;
    }
    await lock.setHardLock(enabled);
  }

  Future<String?> _askNewPassphrase() {
    return showDialog<String>(
      context: context,
      builder: (context) => const _NewPassphraseDialog(),
    );
  }

  Future<bool?> _confirmPassphrase() {
    return showDialog<bool>(
      context: context,
      builder: (context) => _ConfirmPassphraseDialog(
        matches: widget.controller.appLock.passphraseMatches,
      ),
    );
  }

  Future<void> _saveName(String value) async {
    try {
      await widget.controller.setDisplayName(value);
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Display name saved')));
    } catch (err) {
      if (!mounted) return;
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(SnackBar(content: Text(err.toString())));
    }
  }
}

class _NewPassphraseDialog extends StatefulWidget {
  const _NewPassphraseDialog();

  @override
  State<_NewPassphraseDialog> createState() => _NewPassphraseDialogState();
}

class _NewPassphraseDialogState extends State<_NewPassphraseDialog> {
  final _first = TextEditingController();
  final _second = TextEditingController();
  String? _error;

  @override
  void dispose() {
    _first.clear();
    _second.clear();
    _first.dispose();
    _second.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('PIN or passphrase'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          TextField(
            controller: _first,
            obscureText: true,
            decoration: const InputDecoration(labelText: 'PIN or passphrase'),
          ),
          TextField(
            controller: _second,
            obscureText: true,
            decoration: InputDecoration(labelText: 'Again', errorText: _error),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () {
            if (_first.text != _second.text) {
              setState(() => _error = 'those did not match');
              return;
            }
            if (_first.text.length < 4 || _first.text.length > 128) {
              setState(() => _error = 'use 4–128 characters');
              return;
            }
            Navigator.pop(context, _first.text);
          },
          child: const Text('Save'),
        ),
      ],
    );
  }
}

class _ConfirmPassphraseDialog extends StatefulWidget {
  const _ConfirmPassphraseDialog({required this.matches});

  final bool Function(String passphrase) matches;

  @override
  State<_ConfirmPassphraseDialog> createState() =>
      _ConfirmPassphraseDialogState();
}

class _ConfirmPassphraseDialogState extends State<_ConfirmPassphraseDialog> {
  final _field = TextEditingController();
  String? _error;

  @override
  void dispose() {
    _field.clear();
    _field.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Confirm passphrase'),
      content: TextField(
        controller: _field,
        obscureText: true,
        decoration: InputDecoration(errorText: _error),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context, false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () {
            if (!widget.matches(_field.text)) {
              setState(() => _error = 'Wrong passphrase.');
              return;
            }
            Navigator.pop(context, true);
          },
          child: const Text('Confirm'),
        ),
      ],
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: Theme.of(context).textTheme.labelMedium),
          const SizedBox(height: 2),
          SelectableText(value),
        ],
      ),
    );
  }
}

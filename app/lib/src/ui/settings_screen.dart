import 'package:flutter/material.dart';

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
      listenable: widget.controller,
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
                ? 'Running · ${network.connectedPeers} peers'
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
            'Calls are not routed through Tor. With Tor on, a call reveals your IP address to the other person unless you force a relay.',
            style: theme.textTheme.bodyMedium?.copyWith(height: 1.4),
          ),
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

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';

class ContactDetailScreen extends StatelessWidget {
  const ContactDetailScreen({
    super.key,
    required this.controller,
    required this.peerId,
  });

  final UmbraController controller;
  final String peerId;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final contact = controller.contactFor(peerId);
    if (contact == null) {
      return const Scaffold(body: Center(child: Text('Contact removed')));
    }
    final theme = Theme.of(context);
    final groups = contact.safetyNumber.split(' ');
    return Scaffold(
      appBar: AppBar(title: Text(contact.label)),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(20, 12, 20, 32),
        children: [
          Text(contact.displayName, style: theme.textTheme.headlineSmall),
          const SizedBox(height: 4),
          Text(
            contact.online ? 'Online' : 'Offline',
            style: TextStyle(
              color: contact.online ? UmbraColors.online : UmbraColors.mist,
            ),
          ),
          const SizedBox(height: 16),
          SelectableText(contact.peerId, style: theme.textTheme.bodySmall),
          const SizedBox(height: 28),
          Text('Safety number', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          Text(
            'Compare these 12 groups in person or on a call. They are the same on both devices when you are talking to the person who holds the matching keys.',
            style: theme.textTheme.bodyMedium?.copyWith(height: 1.4),
          ),
          const SizedBox(height: 12),
          SelectableText(
            groups.join('  '),
            style: theme.textTheme.bodyLarge?.copyWith(
              fontFeatures: const [FontFeature.tabularFigures()],
              letterSpacing: 0.6,
              height: 1.6,
              fontFamily: 'monospace',
            ),
          ),
          const SizedBox(height: 8),
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: () {
                Clipboard.setData(ClipboardData(text: contact.safetyNumber));
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Safety number copied')),
                );
              },
              icon: const Icon(Icons.copy, size: 18),
              label: const Text('Copy'),
            ),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Verified'),
            subtitle: const Text(
              'Mark this after you have compared the safety number.',
            ),
            value: contact.verified,
            onChanged: (value) {
              controller.setVerified(contact.peerId, verified: value);
            },
          ),
          const SizedBox(height: 12),
          OutlinedButton(
            onPressed: () async {
              final remove = await showDialog<bool>(
                context: context,
                builder: (context) => AlertDialog(
                  title: const Text('Remove contact?'),
                  content: const Text(
                    'This device will forget the chat. The other person is not notified.',
                  ),
                  actions: [
                    TextButton(
                      onPressed: () => Navigator.pop(context, false),
                      child: const Text('Cancel'),
                    ),
                    FilledButton(
                      onPressed: () => Navigator.pop(context, true),
                      child: const Text('Remove'),
                    ),
                  ],
                ),
              );
              if (remove != true || !context.mounted) return;
              await controller.removeContact(contact.peerId);
              if (context.mounted) {
                Navigator.of(context).popUntil((route) => route.isFirst);
              }
            },
            child: const Text('Remove contact'),
          ),
        ],
      ),
    );
  }
}

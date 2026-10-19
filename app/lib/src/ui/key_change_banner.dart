import 'package:flutter/material.dart';

import '../core/core_errors.dart';
import '../models/core_types.dart';
import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';

class KeyChangeBanner extends StatefulWidget {
  const KeyChangeBanner({
    super.key,
    required this.controller,
    required this.notice,
  });

  final UmbraController controller;
  final KeyChangeNotice notice;

  @override
  State<KeyChangeBanner> createState() => _KeyChangeBannerState();
}

class _KeyChangeBannerState extends State<KeyChangeBanner> {
  var _busy = false;
  String? _error;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final contact = widget.controller.contactFor(widget.notice.peerId);
    final name = contact?.label ?? widget.notice.peerId;
    final newNumber = widget.notice.newSafetyNumber.trim();
    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.fromLTRB(16, 14, 16, 8),
      decoration: BoxDecoration(
        color: UmbraColors.danger.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: UmbraColors.danger),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Safety number changed',
            style: theme.textTheme.titleMedium?.copyWith(
              color: UmbraColors.danger,
            ),
          ),
          const SizedBox(height: 6),
          Text(
            '$name presented a new identity key. Compare the new safety number in person before you accept. Accepting resets verified.',
            style: theme.textTheme.bodyMedium?.copyWith(height: 1.35),
          ),
          const SizedBox(height: 6),
          Text(
            keyChangeSourceCopy(widget.notice.source),
            style: theme.textTheme.bodySmall,
          ),
          const SizedBox(height: 10),
          _Number(label: 'Previous', value: widget.notice.oldSafetyNumber),
          if (newNumber.isNotEmpty) ...[
            const SizedBox(height: 8),
            _Number(label: 'New', value: newNumber),
          ],
          if (_error != null) ...[
            const SizedBox(height: 8),
            Text(_error!, style: TextStyle(color: theme.colorScheme.error)),
          ],
          Row(
            children: [
              TextButton(
                onPressed: _busy ? null : () => _act(accept: false),
                child: const Text('Reject'),
              ),
              const Spacer(),
              FilledButton(
                onPressed: _busy ? null : () => _act(accept: true),
                child: const Text('Accept'),
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _act({required bool accept}) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      if (accept) {
        await widget.controller.acceptKeyChange(widget.notice.peerId);
      } else {
        await widget.controller.rejectKeyChange(widget.notice.peerId);
      }
    } catch (err) {
      if (!mounted) return;
      final raw = err.toString();
      setState(() {
        _busy = false;
        _error = coreErrorMayContainAddress(raw)
            ? "couldn't update this contact"
            : raw;
      });
    }
  }
}

/// Where the new key was seen. Roster is a server member list.
String keyChangeSourceCopy(KeyChangeSource source) {
  return switch (source) {
    KeyChangeSource.hello => 'Seen in a hello.',
    KeyChangeSource.keyClaim => 'Seen in a key claim.',
    KeyChangeSource.contactCard => 'Seen on a contact card.',
    KeyChangeSource.roster => 'Seen in a server roster.',
  };
}

class _Number extends StatelessWidget {
  const _Number({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(label, style: Theme.of(context).textTheme.labelMedium),
        const SizedBox(height: 2),
        SelectableText(
          value,
          style: const TextStyle(
            fontFamily: 'monospace',
            fontFeatures: [FontFeature.tabularFigures()],
            height: 1.4,
          ),
        ),
      ],
    );
  }
}

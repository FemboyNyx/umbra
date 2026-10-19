import 'package:flutter/material.dart';

import '../core/core_errors.dart';
import '../models/core_types.dart';
import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';

/// Collapsed count. A bare number would look like an unread badge.
String messageRequestCountLabel(int count) {
  if (count == 1) return '1 request';
  return '$count requests';
}

/// DMs from co-members who are not contacts. Separate from the inbox.
class MessageRequestsScreen extends StatelessWidget {
  const MessageRequestsScreen({super.key, required this.controller});

  final UmbraController controller;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final requests = controller.messageRequests;
        return Scaffold(
          appBar: AppBar(title: const Text('Message requests')),
          body: requests.isEmpty
              ? Center(
                  child: Text(
                    'No message requests.',
                    style: Theme.of(
                      context,
                    ).textTheme.bodyMedium?.copyWith(color: UmbraColors.mist),
                  ),
                )
              : ListView(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
                  children: [
                    for (final request in requests)
                      _RequestTile(controller: controller, request: request),
                  ],
                ),
        );
      },
    );
  }
}

class _RequestTile extends StatefulWidget {
  const _RequestTile({required this.controller, required this.request});

  final UmbraController controller;
  final MessageRequest request;

  @override
  State<_RequestTile> createState() => _RequestTileState();
}

class _RequestTileState extends State<_RequestTile> {
  var _busy = false;
  String? _error;

  @override
  Widget build(BuildContext context) {
    final request = widget.request;
    final theme = Theme.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 14, 16, 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(request.label, style: theme.textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              'From ${request.serverName}',
              style: theme.textTheme.bodySmall?.copyWith(
                color: UmbraColors.mist,
              ),
            ),
            const SizedBox(height: 8),
            Text(request.body),
            if (_error != null) ...[
              const SizedBox(height: 8),
              Text(
                _error!,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: UmbraColors.danger,
                ),
              ),
            ],
            Row(
              children: [
                TextButton(
                  onPressed: _busy
                      ? null
                      : () => _run(
                          () => widget.controller.dismissMessageRequest(
                            request.id,
                          ),
                        ),
                  child: const Text('Not now'),
                ),
                const Spacer(),
                FilledButton(
                  onPressed: _busy
                      ? null
                      : () => _run(
                          () => widget.controller.acceptMessageRequest(
                            request.id,
                          ),
                        ),
                  child: const Text('Accept'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  Future<void> _run(Future<void> Function() action) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await action();
    } catch (err, stack) {
      debugLogCoreError(err, stack);
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

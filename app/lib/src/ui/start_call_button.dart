import 'dart:async';

import 'package:flutter/material.dart';
import 'package:umbra_calls/umbra_calls.dart' show torRelayRequiredMessage;

import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';

/// App-bar action that starts an outgoing call for [peerId].
class StartCallButton extends StatelessWidget {
  const StartCallButton({
    super.key,
    required this.controller,
    required this.peerId,
  });

  final UmbraController controller;
  final String peerId;

  @override
  Widget build(BuildContext context) {
    final enabled = controller.canStartCall(peerId);
    return IconButton(
      tooltip: controller.startCallTooltip(peerId),
      onPressed: enabled
          ? () => unawaited(controller.startOutgoingCall(peerId))
          : null,
      icon: const Icon(Icons.call),
    );
  }
}

/// Calling banner, audio-device failure, and the Tor block for one peer.
class CallStatusBanner extends StatelessWidget {
  const CallStatusBanner({
    super.key,
    required this.controller,
    required this.peerId,
  });

  final UmbraController controller;
  final String peerId;

  @override
  Widget build(BuildContext context) {
    final calling = controller.activeCallPeerId == peerId;
    final notice = controller.callNotice;
    final tor = controller.callsBlockedByTor;
    if (!calling && notice == null && !tor) return const SizedBox.shrink();
    return Padding(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
      child: Column(
        children: [
          if (tor)
            _CallCard(
              title: torRelayRequiredMessage,
              body:
                  'Calls are relay-only in Tor mode and stay blocked until a TURN server is configured.',
            ),
          if (notice != null) ...[
            if (tor) const SizedBox(height: 8),
            _CallCard(
              title: notice,
              action: TextButton(
                onPressed: controller.dismissCallNotice,
                child: const Text('Dismiss'),
              ),
            ),
          ],
          if (calling) ...[
            if (tor || notice != null) const SizedBox(height: 8),
            _CallCard(
              title: 'Calling…',
              alert: false,
              action: TextButton(
                onPressed: () => unawaited(controller.cancelOutgoingCall()),
                child: const Text('Cancel'),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _CallCard extends StatelessWidget {
  const _CallCard({
    required this.title,
    this.body,
    this.action,
    this.alert = true,
  });

  final String title;
  final String? body;
  final Widget? action;
  final bool alert;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: alert
          ? UmbraColors.danger.withValues(alpha: 0.12)
          : UmbraColors.surfaceHigh,
      margin: EdgeInsets.zero,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleSmall),
            if (body != null) ...[const SizedBox(height: 4), Text(body!)],
            if (action != null)
              Align(alignment: Alignment.centerRight, child: action),
          ],
        ),
      ),
    );
  }
}

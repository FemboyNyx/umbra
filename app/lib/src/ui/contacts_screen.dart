import 'package:flutter/material.dart';
import 'package:umbra_calls/umbra_calls.dart' show torRelayRequiredMessage;

import '../models/core_types.dart';
import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';
import 'chat_screen.dart';
import 'key_change_banner.dart';
import 'message_requests_screen.dart';
import 'settings_screen.dart';

class ContactsScreen extends StatelessWidget {
  const ContactsScreen({super.key, required this.controller});

  final UmbraController controller;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: const Text('Umbra'),
        actions: [
          if (controller.settings.mock)
            const Padding(
              padding: EdgeInsets.only(right: 4),
              child: Chip(label: Text('Mock')),
            ),
          IconButton(
            tooltip: 'Settings',
            onPressed: () {
              Navigator.of(context).push(
                MaterialPageRoute<void>(
                  builder: (_) => SettingsScreen(controller: controller),
                ),
              );
            },
            icon: const Icon(Icons.settings_outlined),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: () => _pasteInvite(context),
        icon: const Icon(Icons.qr_code_2),
        label: const Text('Add'),
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 96),
        children: [
          if (controller.callsBlockedByTor)
            _Notice(
              title: torRelayRequiredMessage,
              body:
                  'Calls are relay-only in Tor mode and stay blocked until a TURN server is configured.',
            ),
          if (controller.callNotice != null)
            _Notice(
              title: controller.callNotice!,
              action: TextButton(
                onPressed: controller.dismissCallNotice,
                child: const Text('Dismiss'),
              ),
            ),
          for (var i = 0; i < controller.missedCallNotices.length; i++)
            _Notice(
              title: 'Someone tried to call',
              body: controller.missedCallNotices[i].peerId,
              detail:
                  controller.missedCallNotices[i].reason == 'tor_relay_required'
                  ? torRelayRequiredMessage
                  : null,
              action: TextButton(
                onPressed: () => controller.dismissMissedCallAt(i),
                child: const Text('Dismiss'),
              ),
            ),
          for (final notice in controller.keyChangeNotices)
            KeyChangeBanner(controller: controller, notice: notice),
          if (controller.network.running)
            Text(
              controller.settings.mock
                  ? 'Mock network is up. A sample contact request arrives on its own.'
                  : 'Network is running.',
              style: theme.textTheme.bodySmall?.copyWith(
                color: UmbraColors.mist,
              ),
            ),
          if (controller.pending.isNotEmpty) ...[
            const SizedBox(height: 16),
            Text('Contact requests', style: theme.textTheme.titleMedium),
            const SizedBox(height: 8),
            for (final card in controller.pending)
              _RequestCard(controller: controller, card: card),
          ],
          if (controller.messageRequests.isNotEmpty) ...[
            const SizedBox(height: 16),
            _MessageRequestRow(controller: controller),
          ],
          const SizedBox(height: 16),
          Text('Chats', style: theme.textTheme.titleMedium),
          const SizedBox(height: 8),
          if (controller.conversations.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 28),
              child: Text(
                'No chats yet. Accept a request, or paste an umbra://add link.',
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: UmbraColors.mist,
                ),
              ),
            )
          else
            for (final conversation in controller.conversations)
              _ConversationTile(
                controller: controller,
                conversation: conversation,
              ),
        ],
      ),
    );
  }

  Future<void> _pasteInvite(BuildContext context) async {
    final payload = TextEditingController();
    final added = await showDialog<bool>(
      context: context,
      builder: (context) {
        String? error;
        var busy = false;
        return StatefulBuilder(
          builder: (context, setLocal) {
            return AlertDialog(
              title: const Text('Add a contact'),
              content: TextField(
                controller: payload,
                minLines: 2,
                maxLines: 4,
                decoration: InputDecoration(
                  hintText: 'umbra://add?peer=…',
                  errorText: error,
                ),
              ),
              actions: [
                TextButton(
                  onPressed: () => Navigator.pop(context, false),
                  child: const Text('Cancel'),
                ),
                FilledButton(
                  onPressed: busy
                      ? null
                      : () async {
                          setLocal(() {
                            busy = true;
                            error = null;
                          });
                          try {
                            await controller.addFromQr(payload.text);
                            if (context.mounted) Navigator.pop(context, true);
                          } catch (err) {
                            setLocal(() {
                              busy = false;
                              error = err.toString();
                            });
                          }
                        },
                  child: const Text('Add'),
                ),
              ],
            );
          },
        );
      },
    );
    payload.dispose();
    if (added == true && context.mounted) {
      ScaffoldMessenger.of(
        context,
      ).showSnackBar(const SnackBar(content: Text('Contact added')));
    }
  }
}

class _MessageRequestRow extends StatelessWidget {
  const _MessageRequestRow({required this.controller});

  final UmbraController controller;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        title: const Text('Message requests'),
        subtitle: Text(
          messageRequestCountLabel(controller.messageRequests.length),
        ),
        trailing: const Icon(Icons.chevron_right),
        onTap: () {
          Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => MessageRequestsScreen(controller: controller),
            ),
          );
        },
      ),
    );
  }
}

class _RequestCard extends StatelessWidget {
  const _RequestCard({required this.controller, required this.card});

  final UmbraController controller;
  final ContactCard card;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 14, 16, 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(card.label, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(
              card.peerId,
              maxLines: 1,
              overflow: TextOverflow.ellipsis,
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 4),
            Row(
              children: [
                TextButton(
                  onPressed: () => controller.dismissRequest(card),
                  child: const Text('Ignore'),
                ),
                const Spacer(),
                FilledButton(
                  onPressed: () => controller.acceptRequest(card),
                  child: const Text('Accept'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _ConversationTile extends StatelessWidget {
  const _ConversationTile({
    required this.controller,
    required this.conversation,
  });

  final UmbraController controller;
  final ConversationSummary conversation;

  @override
  Widget build(BuildContext context) {
    final contact = controller.contactFor(conversation.peerId);
    final last = conversation.lastMessage;
    return Card(
      child: ListTile(
        leading: _Presence(online: contact?.online ?? false),
        title: Text(conversation.label),
        subtitle: Text(
          last?.body ?? 'No messages yet',
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
        ),
        trailing: conversation.unreadCount == 0
            ? (contact?.keyChanged ?? false)
                  ? const Icon(Icons.warning_amber, color: UmbraColors.danger)
                  : (contact?.verified ?? false)
                  ? const Icon(Icons.verified_user, size: 18)
                  : null
            : CircleAvatar(
                radius: 12,
                child: Text(
                  '${conversation.unreadCount}',
                  style: const TextStyle(fontSize: 12),
                ),
              ),
        onTap: () {
          Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (_) => ChatScreen(
                controller: controller,
                peerId: conversation.peerId,
              ),
            ),
          );
        },
      ),
    );
  }
}

class _Notice extends StatelessWidget {
  const _Notice({required this.title, this.body, this.detail, this.action});

  final String title;
  final String? body;
  final String? detail;
  final Widget? action;

  @override
  Widget build(BuildContext context) {
    return Card(
      color: UmbraColors.danger.withValues(alpha: 0.12),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 8),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title, style: Theme.of(context).textTheme.titleSmall),
            if (body != null) ...[const SizedBox(height: 4), Text(body!)],
            if (detail != null) ...[
              const SizedBox(height: 4),
              Text(detail!, style: Theme.of(context).textTheme.bodySmall),
            ],
            if (action != null)
              Align(alignment: Alignment.centerRight, child: action),
          ],
        ),
      ),
    );
  }
}

class _Presence extends StatelessWidget {
  const _Presence({required this.online});

  final bool online;

  @override
  Widget build(BuildContext context) {
    return CircleAvatar(
      backgroundColor: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: Icon(
        Icons.person,
        color: online ? UmbraColors.online : UmbraColors.mist,
      ),
    );
  }
}

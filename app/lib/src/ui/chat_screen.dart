import 'dart:async';

import 'package:flutter/material.dart';

import '../models/core_types.dart';
import '../state/umbra_controller.dart';
import '../theme/umbra_theme.dart';
import 'contact_detail_screen.dart';
import 'key_change_banner.dart';

class ChatScreen extends StatefulWidget {
  const ChatScreen({super.key, required this.controller, required this.peerId});

  final UmbraController controller;
  final String peerId;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

class _ChatScreenState extends State<ChatScreen> {
  final _body = TextEditingController();
  final _scroll = ScrollController();
  final _focus = FocusNode();
  String? _error;

  @override
  void initState() {
    super.initState();
    widget.controller.setThreadOpen(widget.peerId, open: true);
  }

  @override
  void dispose() {
    widget.controller.setThreadOpen(widget.peerId, open: false);
    _focus.dispose();
    _body.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _send() async {
    final text = _body.text;
    if (text.trim().isEmpty) return;
    setState(() => _error = null);
    try {
      await widget.controller.sendText(widget.peerId, text);
      _body.clear();
      _jumpToEnd();
    } catch (err) {
      setState(() => _error = err.toString());
    } finally {
      _keepFocus();
    }
  }

  void _keepFocus() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) _focus.requestFocus();
    });
  }

  void _jumpToEnd() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!_scroll.hasClients) return;
      _scroll.animateTo(
        _scroll.position.maxScrollExtent,
        duration: const Duration(milliseconds: 180),
        curve: Curves.easeOut,
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.controller,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final contact = widget.controller.contactFor(widget.peerId);
    final messages = widget.controller.messagesFor(widget.peerId);
    final title = contact?.label ?? widget.peerId;
    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(title),
            Text(
              contact == null
                  ? ''
                  : contact.online
                  ? 'Online'
                  : 'Offline',
              style: Theme.of(context).textTheme.labelSmall,
            ),
          ],
        ),
        actions: [
          IconButton(
            tooltip: 'Contact details',
            onPressed: contact == null
                ? null
                : () {
                    Navigator.of(context).push(
                      MaterialPageRoute<void>(
                        builder: (_) => ContactDetailScreen(
                          controller: widget.controller,
                          peerId: widget.peerId,
                        ),
                      ),
                    );
                  },
            icon: const Icon(Icons.info_outline),
          ),
        ],
      ),
      body: Column(
        children: [
          for (final notice in widget.controller.keyChangeNotices)
            if (notice.peerId == widget.peerId)
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 12, 12, 0),
                child: KeyChangeBanner(
                  controller: widget.controller,
                  notice: notice,
                ),
              ),
          Expanded(
            child: messages.isEmpty
                ? const Center(child: Text('Say hello.'))
                : ListView.builder(
                    controller: _scroll,
                    padding: const EdgeInsets.fromLTRB(12, 12, 12, 12),
                    itemCount: messages.length,
                    itemBuilder: (context, index) {
                      return _Bubble(message: messages[index]);
                    },
                  ),
          ),
          if (_error != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 16),
              child: Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          SafeArea(
            top: false,
            child: Padding(
              padding: const EdgeInsets.fromLTRB(12, 4, 12, 12),
              child: Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _body,
                      focusNode: _focus,
                      minLines: 1,
                      maxLines: 4,
                      textInputAction: TextInputAction.send,
                      onSubmitted: (_) => _send(),
                      decoration: const InputDecoration(hintText: 'Message'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  IconButton.filled(
                    tooltip: 'Send',
                    onPressed: _send,
                    icon: const Icon(Icons.send),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _Bubble extends StatelessWidget {
  const _Bubble({required this.message});

  final Message message;

  @override
  Widget build(BuildContext context) {
    final outgoing = message.outgoing;
    final scheme = Theme.of(context).colorScheme;
    final align = outgoing ? Alignment.centerRight : Alignment.centerLeft;
    final color = outgoing ? scheme.primary : UmbraColors.surfaceHigh;
    final onColor = outgoing ? scheme.onPrimary : scheme.onSurface;
    return Align(
      alignment: align,
      child: Container(
        constraints: const BoxConstraints(maxWidth: 520),
        margin: const EdgeInsets.symmetric(vertical: 4),
        padding: const EdgeInsets.fromLTRB(14, 10, 14, 8),
        decoration: BoxDecoration(
          color: color,
          borderRadius: BorderRadius.circular(16),
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.end,
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: Text(message.body, style: TextStyle(color: onColor)),
            ),
            const SizedBox(height: 4),
            Text(
              _caption(message),
              style: TextStyle(
                color: onColor.withValues(alpha: 0.75),
                fontSize: 11,
              ),
            ),
          ],
        ),
      ),
    );
  }

  static String _caption(Message message) {
    final time = DateTime.fromMillisecondsSinceEpoch(message.timestampMs);
    final clock =
        '${time.hour.toString().padLeft(2, '0')}:${time.minute.toString().padLeft(2, '0')}';
    if (!message.outgoing) return clock;
    final status = switch (message.status) {
      DeliveryStatus.sent => 'Sent',
      DeliveryStatus.delivered => 'Delivered',
      DeliveryStatus.failed => 'Failed',
      DeliveryStatus.received => 'Received',
    };
    return '$clock · $status';
  }
}

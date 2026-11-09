import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../api.dart';
import '../draft.dart';
import 'result_view.dart';

class _Msg {
  _Msg.user(this.text)
    : mine = true,
      view = null,
      result = null,
      pdf = null,
      error = false;
  _Msg.bot(this.text, {this.view, this.result, this.pdf, this.error = false})
    : mine = false;

  final bool mine;
  final String text;
  final CalcView? view;
  final Json? result;
  final String? pdf;
  final bool error;
}

/// Chat with the assistant. The assistant understands the request and drives the
/// calculation engine; the tables under each reply are the engine's own output,
/// shown by the same code as the New order screen.
class AssistantPage extends StatefulWidget {
  const AssistantPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<AssistantPage> createState() => _AssistantPageState();
}

class _AssistantPageState extends State<AssistantPage> {
  final _input = TextEditingController();
  final _scroll = ScrollController();
  final _msgs = <_Msg>[];
  String? _session;
  List<String> _order = const [];
  bool _busy = false;
  bool? _enabled;

  ApiClient get api => widget.controller.api;

  @override
  void initState() {
    super.initState();
    _checkStatus();
  }

  Future<void> _checkStatus() async {
    try {
      final on = await api.assistantEnabled();
      if (mounted) setState(() => _enabled = on);
    } on ApiException {
      if (mounted) setState(() => _enabled = null);
    }
  }

  @override
  void dispose() {
    _input.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _send([String? preset]) async {
    final text = (preset ?? _input.text).trim();
    if (text.isEmpty || _busy) return;
    _input.clear();
    setState(() {
      _msgs.add(_Msg.user(text));
      _busy = true;
    });
    _toEnd();
    try {
      final r = await api.assistantChat(_session, text);
      _session = r['session_id'] as String?;
      _order = ((r['order'] as List?) ?? const []).map((e) => '$e').toList();
      final res = r['result'] as Json?;
      setState(
        () => _msgs.add(
          _Msg.bot(
            '${r['reply']}',
            view: res == null
                ? null
                : CalcView.fromWire(res['view'] as String?),
            result: res?['result'] as Json?,
            pdf: r['quote_pdf_path'] as String?,
          ),
        ),
      );
    } on ApiException catch (e) {
      setState(() => _msgs.add(_Msg.bot(e.message, error: true)));
    } finally {
      if (mounted) setState(() => _busy = false);
      _toEnd();
    }
  }

  void _toEnd() => WidgetsBinding.instance.addPostFrameCallback((_) {
    if (_scroll.hasClients) {
      _scroll.animateTo(
        _scroll.position.maxScrollExtent,
        duration: const Duration(milliseconds: 200),
        curve: Curves.easeOut,
      );
    }
  });

  void _newChat() => setState(() {
    _msgs.clear();
    _session = null;
    _order = const [];
  });

  Future<void> _openPdf(String path) async {
    try {
      await launchUrl(
        await api.ticketedUri(path),
        mode: LaunchMode.externalApplication,
      );
    } on ApiException catch (e) {
      if (mounted) setState(() => _msgs.add(_Msg.bot(e.message, error: true)));
    }
  }

  static const _examples = [
    'Calculate 10 windows measuring 4 x 4 feet, 5 windows measuring 5 x 4 feet and 3 toilet windows measuring 2 x 2 feet.',
    'Show me rubber consumption only.',
    'How many 21-ft bars should I purchase?',
    'Calculate the full production cost.',
  ];

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context);
    if (_enabled == false) {
      return Padding(
        padding: const EdgeInsets.all(24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('AI assistant', style: t.textTheme.titleLarge),
            const SizedBox(height: 12),
            const Text(
              'The assistant is switched off. Put an AI key (OPENAI_API_KEY, GEMINI_API_KEY, GROQ_API_KEY or ANTHROPIC_API_KEY) in the .env file and restart the server to turn it on. '
              'Everything else in the app works without it.',
            ),
            const SizedBox(height: 12),
            OutlinedButton(
              onPressed: _checkStatus,
              child: const Text('Check again'),
            ),
          ],
        ),
      );
    }
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 8, 0),
          child: Row(
            children: [
              Expanded(
                child: Text('AI assistant', style: t.textTheme.titleLarge),
              ),
              TextButton.icon(
                onPressed: _newChat,
                icon: const Icon(Icons.add_comment_outlined),
                label: const Text('New chat'),
              ),
            ],
          ),
        ),
        if (_order.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Align(
              alignment: Alignment.centerLeft,
              child: Text(
                'Current order: ${_order.join('   |   ')}',
                style: t.textTheme.bodySmall,
              ),
            ),
          ),
        Expanded(
          child: _msgs.isEmpty
              ? ListView(
                  padding: const EdgeInsets.all(16),
                  children: [
                    const Text(
                      'Tell me the windows in plain words. I send them to the calculation engine; I never do the sums myself. Try:',
                    ),
                    const SizedBox(height: 8),
                    for (final e in _examples)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 6),
                        child: ActionChip(
                          label: Text(e, softWrap: true),
                          onPressed: () => _send(e),
                        ),
                      ),
                  ],
                )
              : ListView.builder(
                  controller: _scroll,
                  padding: const EdgeInsets.all(16),
                  itemCount: _msgs.length,
                  itemBuilder: (context, i) {
                    final m = _msgs[i];
                    return Align(
                      alignment: m.mine
                          ? Alignment.centerRight
                          : Alignment.centerLeft,
                      child: Container(
                        margin: const EdgeInsets.only(bottom: 12),
                        padding: const EdgeInsets.all(12),
                        constraints: BoxConstraints(
                          maxWidth: m.mine ? 520 : 900,
                        ),
                        decoration: BoxDecoration(
                          color: m.error
                              ? t.colorScheme.errorContainer
                              : (m.mine
                                    ? t.colorScheme.primaryContainer
                                    : t.colorScheme.surfaceContainerHighest),
                          borderRadius: BorderRadius.circular(12),
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            SelectableText(m.text),
                            if (m.result != null) ...[
                              const SizedBox(height: 12),
                              ResultView(
                                view: m.view ?? CalcView.standard,
                                result: m.result!,
                              ),
                            ],
                            if (m.pdf != null)
                              FilledButton.icon(
                                onPressed: () => _openPdf(m.pdf!),
                                icon: const Icon(Icons.picture_as_pdf),
                                label: const Text('Open quotation PDF'),
                              ),
                          ],
                        ),
                      ),
                    );
                  },
                ),
        ),
        if (_busy) const LinearProgressIndicator(),
        Padding(
          padding: const EdgeInsets.all(12),
          child: Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _input,
                  minLines: 1,
                  maxLines: 4,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => _send(),
                  decoration: const InputDecoration(
                    hintText: 'e.g. 5 windows 4 x 4 ft and 3 windows 5 x 4 ft',
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton.filled(
                onPressed: _busy ? null : _send,
                icon: const Icon(Icons.send),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

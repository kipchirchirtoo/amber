import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';
import '../format.dart';
import 'result_view.dart';
import 'account_sections.dart';
import 'business_section.dart';
import 'rules_editor.dart';

/// Rules in force, price editing (admin PIN) and the full change history.
class SettingsPage extends StatefulWidget {
  const SettingsPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends State<SettingsPage> {
  late final _url = TextEditingController(text: widget.controller.api.baseUrl);
  late Future<List<Json>> _history = widget.controller.api.rulesHistory();
  String? _status;

  Future<void> _connect() async {
    widget.controller.api.baseUrl = _url.text.trim();
    try {
      await widget.controller.api.health();
      setState(() {
        _status = 'Connected.';
        _history = widget.controller.api.rulesHistory();
      });
    } on ApiException catch (e) {
      setState(() => _status = e.message);
    }
  }

  @override
  void dispose() {
    _url.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Text('Settings', style: Theme.of(context).textTheme.titleLarge),
        const SizedBox(height: 16),
        Row(
          children: [
            Expanded(
              child: TextField(
                controller: _url,
                decoration: const InputDecoration(
                  labelText: 'Server address',
                  border: OutlineInputBorder(),
                ),
                onSubmitted: (_) => _connect(),
              ),
            ),
            const SizedBox(width: 8),
            FilledButton(onPressed: _connect, child: const Text('Connect')),
          ],
        ),
        if (_status != null)
          Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(_status!),
          ),
        const SizedBox(height: 24),
        Section(
          title: 'Your account',
          child: AccountSection(controller: widget.controller),
        ),
        if (widget.controller.isAdmin)
          Section(
            title: 'Users (administrator)',
            child: UsersSection(controller: widget.controller),
          ),
        Section(
          title: widget.controller.isAdmin
              ? 'Quotation letterhead and payment details'
              : 'Quotation letterhead and payment details (administrator edits)',
          child: BusinessSection(controller: widget.controller),
        ),
        FutureBuilder<List<Json>>(
          future: _history,
          builder: (context, snap) {
            if (snap.connectionState != ConnectionState.done) {
              return const Center(child: CircularProgressIndicator());
            }
            if (snap.hasError) return Text('${snap.error}');
            final h = snap.data!;
            final r = h.first['rules'] as Json;
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Section(
                  title: 'Rules in force: version ${h.first['version_no']}',
                  child: Tbl(
                    headers: const ['Setting', 'Value'],
                    rows: [
                      [
                        'Aluminium stock bar',
                        '${fmt(r['stock_length_mm'])} mm at ${ksh(r['stock_price_ksh'])}',
                      ],
                      [
                        'Rubber',
                        '${ksh((r['rubber'] as Json)['rate_per_metre_ksh'])} per metre',
                      ],
                      [
                        'Glass sheet',
                        ksh((r['glass'] as Json)['sheet_price_ksh']),
                      ],
                      for (final f in (r['fittings'] as List).cast<Json>())
                        [
                          fittingLabels[f['kind']] ?? '${f['kind']}',
                          '${f['per_window']} per window at ${ksh(f['unit_price_ksh'])}',
                        ],
                      [
                        'Labour, 2 x 2 ft or smaller',
                        ksh((r['labour'] as Json)['small_rate_ksh']),
                      ],
                      [
                        'Labour, larger',
                        ksh((r['labour'] as Json)['large_rate_ksh']),
                      ],
                    ],
                  ),
                ),
                if (widget.controller.isAdmin)
                  Section(
                    title: 'Edit prices and charges (administrator)',
                    child: RulesEditor(
                      api: widget.controller.api,
                      rules: r,
                      onSaved: () => setState(() {
                        _history = widget.controller.api.rulesHistory();
                      }),
                    ),
                  ),
                Section(
                  title: 'Aluminium cutting formulas (fixed business rules)',
                  child: Tbl(
                    headers: const [
                      'Profile',
                      'Basis',
                      'Adjust (mm)',
                      'Pieces',
                    ],
                    rows: [
                      for (final c in (r['cuts'] as List).cast<Json>())
                        [
                          profileLabels[c['profile']] ?? '${c['profile']}',
                          '${c['basis']}'.replaceAll('_', ' '),
                          fmt(c['adjust_mm'], dp: 0, group: false),
                          '${c['pieces_per_window']}',
                        ],
                    ],
                  ),
                ),
                Section(
                  title: 'Change history',
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      for (final v in h)
                        ListTile(
                          contentPadding: EdgeInsets.zero,
                          dense: true,
                          title: Text(
                            'Version ${v['version_no']}: ${v['note']}',
                          ),
                          subtitle: Text(
                            '${'${v['created_at']}'.split('.').first.replaceAll('T', ' ')}${v['changed_by'] == null ? '' : ' | by ${v['changed_by']}'}',
                          ),
                          trailing: v['authorised_formula_change'] == true
                              ? const StatusChip('Formula change', ok: false)
                              : null,
                        ),
                    ],
                  ),
                ),
              ],
            );
          },
        ),
      ],
    );
  }
}

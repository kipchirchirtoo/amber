import 'package:flutter/material.dart';

import '../api.dart';
import '../format.dart';
import '../theme.dart';

/// Administrator form for prices and charges, plus the measurement deductions.
/// The server stores each save as a NEW rules version, so past quotations and
/// saved orders keep the figures they were calculated with. The measurement
/// formulas (aluminium cut deductions and glass-panel deductions) sit in their
/// own locked section: they only change when the administrator deliberately
/// ticks the authorisation box and confirms, and the version is recorded as an
/// authorised formula change.
class RulesEditor extends StatefulWidget {
  const RulesEditor({
    super.key,
    required this.api,
    required this.rules,
    required this.onSaved,
  });
  final ApiClient api;
  final Json rules;
  final VoidCallback onSaved;

  @override
  State<RulesEditor> createState() => _RulesEditorState();
}

class _Field {
  _Field(this.label, this.holder, this.key, this.initial)
    : controller = TextEditingController(text: initial);
  final String label;
  final Map holder;
  final Object key;
  final TextEditingController controller;
  final String initial;

  bool get changed => controller.text.trim() != initial;
}

class _RulesEditorState extends State<RulesEditor> {
  final _fields = <_Field>[];
  final _formulaFields = <_Field>[];
  final _note = TextEditingController();
  late Json _rules;
  String? _error;
  String? _ok;
  bool _busy = false;
  bool _authorise = false;

  @override
  void initState() {
    super.initState();
    _build();
  }

  void _disposeFields() {
    for (final f in [..._fields, ..._formulaFields]) {
      f.controller.dispose();
    }
    _fields.clear();
    _formulaFields.clear();
  }

  void _build() {
    _disposeFields();
    _authorise = false;
    _rules = _deepCopy(widget.rules) as Json;
    final r = _rules;
    void add(String label, Map holder, String key) =>
        _fields.add(_Field(label, holder, key, '${holder[key]}'));
    add('Aluminium bar length (mm)', r, 'stock_length_mm');
    add('Aluminium bar price (KSh)', r, 'stock_price_ksh');
    add(
      'Rubber rate per metre (KSh)',
      r['rubber'] as Map,
      'rate_per_metre_ksh',
    );
    add('Rubber roll price (KSh)', r['rubber'] as Map, 'roll_price_ksh');
    for (final f in (r['fittings'] as List).cast<Map>()) {
      add(
        '${fittingLabels[f['kind']] ?? f['kind']} price each (KSh)',
        f,
        'unit_price_ksh',
      );
    }
    add(
      'Labour, 2 x 2 ft or smaller (KSh)',
      r['labour'] as Map,
      'small_rate_ksh',
    );
    add('Labour, larger windows (KSh)', r['labour'] as Map, 'large_rate_ksh');
    for (final t in (r['transport'] as List).cast<Map>()) {
      final m = t['max_windows'];
      final band = m == null
          ? 'more than the previous band'
          : (m == 6 ? '1 to 6 windows' : 'up to $m windows');
      add('Transport, $band: from (KSh)', t, 'min_ksh');
      add('Transport, $band: to (KSh)', t, 'max_ksh');
    }
    add('Glass sheet price (KSh)', r['glass'] as Map, 'sheet_price_ksh');

    // Measurement formulas: the deduction in each cut, and the glass panel deductions.
    const basisText = {
      'width': 'width',
      'height': 'height',
      'half_width': 'width ÷ 2',
    };
    for (final c in (r['cuts'] as List).cast<Map>()) {
      _formulaFields.add(
        _Field(
          '${profileLabels[c['profile']] ?? c['profile']}: ${basisText[c['basis']] ?? c['basis']} + (mm)',
          c,
          'adjust_mm',
          '${c['adjust_mm']}',
        ),
      );
    }
    final p = r['panels'] as Map;
    _formulaFields.add(
      _Field(
        'Glass panel width: (width ÷ 2) − (mm)',
        p,
        'width_deduction_mm',
        '${p['width_deduction_mm']}',
      ),
    );
    _formulaFields.add(
      _Field(
        'Glass panel height: height − (mm)',
        p,
        'height_deduction_mm',
        '${p['height_deduction_mm']}',
      ),
    );
  }

  static Object? _deepCopy(Object? o) {
    if (o is Map) {
      return <String, dynamic>{
        for (final e in o.entries) '${e.key}': _deepCopy(e.value),
      };
    }
    if (o is List) return [for (final e in o) _deepCopy(e)];
    return o;
  }

  @override
  void didUpdateWidget(RulesEditor old) {
    super.didUpdateWidget(old);
    if (!identical(old.rules, widget.rules)) _build();
  }

  @override
  void dispose() {
    _disposeFields();
    _note.dispose();
    super.dispose();
  }

  Future<bool> _confirmFormulaChange(List<_Field> changed) async {
    final ok = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        icon: const Icon(Icons.warning_amber_rounded, color: Brand.warn),
        title: const Text('Change the measurement formulas?'),
        content: SizedBox(
          width: 460,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'These are Amber Stores\' fixed cutting and glass rules. '
                'Every new calculation will use the new values. Saved orders '
                'and quotations keep the values they were made with.',
              ),
              const SizedBox(height: 12),
              for (final f in changed)
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Text(
                    '${f.label}: ${f.initial}  →  ${f.controller.text.trim()}',
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx, false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, true),
            child: const Text('Yes, I authorise this change'),
          ),
        ],
      ),
    );
    return ok ?? false;
  }

  Future<void> _save() async {
    setState(() {
      _error = null;
      _ok = null;
    });
    final prices = _fields.where((f) => f.changed).toList();
    final formulas = _formulaFields.where((f) => f.changed).toList();
    if (prices.isEmpty && formulas.isEmpty) {
      return setState(() => _error = 'Nothing has been changed.');
    }
    if (formulas.isNotEmpty && !_authorise) {
      return setState(
        () => _error =
            'You changed a measurement formula. Tick "I authorise changing the measurement formulas" first, or put the formula values back.',
      );
    }
    if (_note.text.trim().isEmpty) {
      return setState(
        () => _error = 'Write a short note saying why this is changing.',
      );
    }
    if (formulas.isNotEmpty && !await _confirmFormulaChange(formulas)) return;
    final changed = [...prices, ...formulas];
    for (final f in changed) {
      f.holder[f.key] = f.controller.text.trim();
    }
    setState(() => _busy = true);
    try {
      final v = await widget.api.updateRules(
        _rules,
        _note.text.trim(),
        authoriseFormulaChange: formulas.isNotEmpty,
      );
      _note.clear();
      setState(
        () => _ok =
            'Saved as version ${v['version_no']}${formulas.isNotEmpty ? ' (authorised formula change)' : ''}. New calculations use it; old orders and quotes are unchanged.',
      );
      widget.onSaved();
    } on ApiException catch (e) {
      // put the old values back so a retry starts from the live rules
      for (final f in changed) {
        f.holder[f.key] = f.initial;
      }
      setState(() => _error = e.message);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Widget _box(_Field f) => SizedBox(
    width: 250,
    child: TextField(
      controller: f.controller,
      keyboardType: const TextInputType.numberWithOptions(
        decimal: true,
        signed: true,
      ),
      decoration: InputDecoration(labelText: f.label, isDense: true),
    ),
  );

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [for (final f in _fields) _box(f)],
        ),
        const SizedBox(height: 14),
        Material(
          color: Brand.badTint.withValues(alpha: 0.5),
          shape: RoundedRectangleBorder(
            side: const BorderSide(color: Brand.border),
            borderRadius: BorderRadius.circular(8),
          ),
          clipBehavior: Clip.antiAlias,
          child: ExpansionTile(
            shape: const Border(),
            collapsedShape: const Border(),
            leading: const Icon(Icons.lock_outline, color: Brand.warn),
            title: const Text(
              'Measurement formulas (fixed business rules)',
              style: TextStyle(fontWeight: FontWeight.w700),
            ),
            subtitle: const Text(
              'Locked. Change only when you deliberately authorise it.',
            ),
            childrenPadding: const EdgeInsets.fromLTRB(16, 0, 16, 16),
            expandedCrossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Each value is the millimetres added to (or taken off) the window '
                'measurement. A deduction is written as a negative number in the '
                'cut rows, for example -20 for "width − 20 mm". Pieces per window '
                'are not editable here.',
              ),
              const SizedBox(height: 12),
              Wrap(
                spacing: 12,
                runSpacing: 12,
                children: [for (final f in _formulaFields) _box(f)],
              ),
              CheckboxListTile(
                contentPadding: EdgeInsets.zero,
                controlAffinity: ListTileControlAffinity.leading,
                value: _authorise,
                onChanged: (v) => setState(() {
                  _authorise = v ?? false;
                }),
                title: const Text(
                  'I authorise changing the measurement formulas',
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 14),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            SizedBox(
              width: 340,
              child: TextField(
                controller: _note,
                decoration: const InputDecoration(
                  labelText: 'Reason for change',
                ),
              ),
            ),
            FilledButton(
              onPressed: _busy ? null : _save,
              child: const Text('Save new version'),
            ),
          ],
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              _error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ),
        if (_ok != null)
          Padding(padding: const EdgeInsets.only(top: 8), child: Text(_ok!)),
      ],
    );
  }
}

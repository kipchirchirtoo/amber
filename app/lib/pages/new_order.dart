import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';
import '../format.dart';
import 'result_view.dart';

class NewOrderPage extends StatefulWidget {
  const NewOrderPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<NewOrderPage> createState() => _NewOrderPageState();
}

class _NewOrderPageState extends State<NewOrderPage> {
  CalcView _view = CalcView.standard;
  CalcView? _shownView;
  Json? _result;
  String? _error;
  bool _busy = false;

  AppController get c => widget.controller;
  OrderDraft get d => c.draft;

  Future<void> _loadSavedStock() async {
    try {
      final items = await c.api.listStock();
      if (!mounted) return;
      if (items.isEmpty) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(
            content: Text('No saved stock. Add some on the Stock screen.'),
          ),
        );
        return;
      }
      // replace blank starter rows, keep anything the user already typed
      d.stock.removeWhere((r) => r.length.text.trim().isEmpty);
      for (final it in items) {
        d.addStockFrom(it);
      }
      setState(() {});
    } on ApiException catch (e) {
      if (mounted) setState(() => _error = e.message);
    }
  }

  Future<void> _calculate([CalcView? v]) async {
    final view = v ?? _view;
    setState(() {
      _view = view;
      _busy = true;
      _error = null;
    });
    try {
      final res = await c.api.calculate(d.toRequest(view));
      if (!mounted) return;
      setState(() {
        _result = res['result'] as Json;
        _shownView = view;
      });
    } on ApiException catch (e) {
      if (mounted) setState(() => _error = e.message);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _save() async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final req = d.toRequest(_view);
      final saved = c.editingOrderId == null
          ? await c.api.createOrder(d.customerName, req)
          : await c.api.updateOrder(c.editingOrderId!, d.customerName, req);
      c.editingOrderId = saved['id'] as String;
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(
          content: Text(
            'Saved as revision ${(saved['latest'] as Json)['revision']} '
            '(rules version ${(saved['latest'] as Json)['rules_version_no']}).',
          ),
        ),
      );
      setState(() {});
    } on ApiException catch (e) {
      if (mounted) setState(() => _error = e.message);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  void _newOrder() {
    c.startNewOrder();
    setState(() {
      _result = null;
      _shownView = null;
      _error = null;
      _view = CalcView.standard;
    });
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: d,
      builder: (context, _) => LayoutBuilder(
        builder: (context, box) {
          final form = _form(context);
          final results = _results(context);
          if (box.maxWidth >= 900) {
            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                SizedBox(
                  width: 440,
                  child: ListView(
                    padding: const EdgeInsets.all(16),
                    children: form,
                  ),
                ),
                const VerticalDivider(width: 1),
                Expanded(
                  child: ListView(
                    padding: const EdgeInsets.all(16),
                    children: results,
                  ),
                ),
              ],
            );
          }
          return ListView(
            padding: const EdgeInsets.all(16),
            children: [...form, const Divider(height: 32), ...results],
          );
        },
      ),
    );
  }

  List<Widget> _form(BuildContext context) {
    return [
      Row(
        children: [
          Expanded(
            child: Text(
              c.editingOrderId == null
                  ? 'New window order'
                  : 'Editing saved order (saves as a new revision)',
              style: Theme.of(context).textTheme.titleLarge,
            ),
          ),
          TextButton.icon(
            onPressed: _busy ? null : _newOrder,
            icon: const Icon(Icons.restart_alt),
            label: const Text('Clear'),
          ),
        ],
      ),
      const SizedBox(height: 12),
      TextField(
        controller: d.customer,
        decoration: const InputDecoration(
          labelText: 'Customer (optional)',
          border: OutlineInputBorder(),
        ),
      ),
      const SizedBox(height: 12),
      DropdownButtonFormField<String?>(
        initialValue: d.defaultUnit,
        decoration: const InputDecoration(
          labelText: 'Unit for sizes typed without a unit',
          border: OutlineInputBorder(),
        ),
        items: const [
          DropdownMenuItem(
            value: null,
            child: Text('None: each size must say its unit'),
          ),
          DropdownMenuItem(value: 'mm', child: Text('Millimetres (mm)')),
          DropdownMenuItem(value: 'ft', child: Text('Feet (ft)')),
          DropdownMenuItem(value: 'm', child: Text('Metres (m)')),
        ],
        onChanged: (v) {
          d.defaultUnit = v;
          d.touch();
        },
      ),
      const SizedBox(height: 16),
      Text(
        'Windows (width x height, or width * height)',
        style: Theme.of(context).textTheme.titleMedium,
      ),
      const SizedBox(height: 4),
      Text(
        'Use x or * between the sizes. Examples: 4 x 4 ft, 4*4 ft, 1200 x 1500 mm, 1.2 x 1.5 m. Width always comes first.',
        style: Theme.of(context).textTheme.bodySmall,
      ),
      const SizedBox(height: 8),
      for (final w in d.windows)
        Padding(
          key: ObjectKey(w),
          padding: const EdgeInsets.only(bottom: 8),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                flex: 3,
                child: TextField(
                  controller: w.size,
                  decoration: const InputDecoration(
                    labelText: 'Size',
                    hintText: '4 x 4 ft',
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) => _calculate(),
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                width: 72,
                child: TextField(
                  controller: w.qty,
                  keyboardType: TextInputType.number,
                  decoration: const InputDecoration(
                    labelText: 'Qty',
                    border: OutlineInputBorder(),
                  ),
                  onSubmitted: (_) => _calculate(),
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                flex: 2,
                child: TextField(
                  controller: w.label,
                  decoration: const InputDecoration(
                    labelText: 'Note',
                    hintText: 'toilet',
                    border: OutlineInputBorder(),
                  ),
                ),
              ),
              IconButton(
                tooltip: 'Remove',
                onPressed: () => d.removeWindow(w),
                icon: const Icon(Icons.close),
              ),
            ],
          ),
        ),
      Align(
        alignment: Alignment.centerLeft,
        child: TextButton.icon(
          onPressed: d.addWindow,
          icon: const Icon(Icons.add),
          label: const Text('Add another size'),
        ),
      ),
      ExpansionTile(
        tilePadding: EdgeInsets.zero,
        title: const Text('More options: transport, existing stock, glass'),
        children: [
          TextField(
            controller: d.transport,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Exact transport (KSh)',
              helperText: 'Leave empty to show the unconfirmed range',
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            controller: d.kerf,
            keyboardType: TextInputType.number,
            decoration: const InputDecoration(
              labelText: 'Saw kerf per cut (mm)',
              helperText: 'Leave empty: no kerf is assumed',
              border: OutlineInputBorder(),
            ),
          ),
          CheckboxListTile(
            contentPadding: EdgeInsets.zero,
            value: d.includeGlass,
            onChanged: (v) {
              d.includeGlass = v ?? false;
              d.touch();
            },
            title: const Text('Include glass sheets in full costing'),
          ),
          Align(
            alignment: Alignment.centerLeft,
            child: Text(
              'Existing aluminium stock',
              style: Theme.of(context).textTheme.titleSmall,
            ),
          ),
          Text(
            'Only stock you tick as confirmed is ever used.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          for (final s in d.stock)
            Padding(
              key: ObjectKey(s),
              padding: const EdgeInsets.only(top: 8),
              child: Row(
                children: [
                  Expanded(
                    flex: 3,
                    child: DropdownButtonFormField<String>(
                      initialValue: s.profile,
                      isExpanded: true,
                      decoration: const InputDecoration(
                        border: OutlineInputBorder(),
                        labelText: 'Profile',
                      ),
                      items: [
                        for (final e in profileLabels.entries)
                          DropdownMenuItem(value: e.key, child: Text(e.value)),
                      ],
                      onChanged: (v) => s.profile = v ?? s.profile,
                    ),
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    flex: 2,
                    child: TextField(
                      controller: s.length,
                      decoration: const InputDecoration(
                        labelText: 'Length',
                        hintText: '8 ft',
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  const SizedBox(width: 6),
                  SizedBox(
                    width: 56,
                    child: TextField(
                      controller: s.qty,
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Qty',
                        border: OutlineInputBorder(),
                      ),
                    ),
                  ),
                  Tooltip(
                    message: 'Confirmed available and usable',
                    child: Checkbox(
                      value: s.confirmed,
                      onChanged: (v) {
                        s.confirmed = v ?? false;
                        d.touch();
                      },
                    ),
                  ),
                  IconButton(
                    onPressed: () => d.removeStock(s),
                    icon: const Icon(Icons.close),
                  ),
                ],
              ),
            ),
          Wrap(
            children: [
              TextButton.icon(
                onPressed: d.addStock,
                icon: const Icon(Icons.add),
                label: const Text('Add existing stock'),
              ),
              TextButton.icon(
                onPressed: _loadSavedStock,
                icon: const Icon(Icons.inventory_2_outlined),
                label: const Text('Load saved stock'),
              ),
            ],
          ),
        ],
      ),
      const SizedBox(height: 12),
      Row(
        children: [
          Expanded(
            child: FilledButton.icon(
              onPressed: _busy ? null : () => _calculate(),
              icon: _busy
                  ? const SizedBox(
                      width: 16,
                      height: 16,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    )
                  : const Icon(Icons.calculate),
              label: const Text('Calculate'),
            ),
          ),
          const SizedBox(width: 8),
          OutlinedButton.icon(
            onPressed: _busy ? null : _save,
            icon: const Icon(Icons.save),
            label: const Text('Save order'),
          ),
        ],
      ),
    ];
  }

  List<Widget> _results(BuildContext context) {
    return [
      Text('Results', style: Theme.of(context).textTheme.titleLarge),
      const SizedBox(height: 8),
      Wrap(
        spacing: 8,
        runSpacing: 4,
        children: [
          for (final v in CalcView.values)
            ChoiceChip(
              label: Text(v.label),
              selected: _view == v,
              onSelected: _busy ? null : (_) => _calculate(v),
            ),
        ],
      ),
      const SizedBox(height: 16),
      if (_error != null)
        Card(
          color: Theme.of(context).colorScheme.errorContainer,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Text(
              _error!,
              style: TextStyle(
                color: Theme.of(context).colorScheme.onErrorContainer,
              ),
            ),
          ),
        ),
      if (_result == null && _error == null)
        const Text(
          'Enter window sizes and press Calculate. The default view shows aluminium requirements and rubber only; use the chips for anything else.',
        ),
      if (_result != null && _shownView != null)
        ResultView(view: _shownView!, result: _result!),
    ];
  }
}

import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';
import '../format.dart';

/// Aluminium stock and offcuts the owner has on hand. Nothing here is assumed
/// usable: an item only counts in a calculation when it is ticked "confirmed".
class StockPage extends StatefulWidget {
  const StockPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<StockPage> createState() => _StockPageState();
}

class _StockPageState extends State<StockPage> {
  late Future<List<Json>> _items = widget.controller.api.listStock();
  final _length = TextEditingController();
  final _qty = TextEditingController(text: '1');
  final _note = TextEditingController();
  String _profile = 'side_track';
  bool _confirmed = false;
  String? _error;

  @override
  void dispose() {
    _length.dispose();
    _qty.dispose();
    _note.dispose();
    super.dispose();
  }

  void _reload() => setState(() {
    _items = widget.controller.api.listStock();
  });

  Future<void> _run(Future<void> Function() job) async {
    try {
      await job();
      setState(() => _error = null);
      _reload();
    } on ApiException catch (e) {
      setState(() => _error = e.message);
    }
  }

  Future<void> _add() => _run(() async {
    final q = int.tryParse(_qty.text.trim());
    if (q == null || q < 1) {
      throw ApiException('Quantity must be a whole number of 1 or more.');
    }
    await widget.controller.api.saveStock(null, {
      'profile': _profile,
      'length': _length.text.trim(),
      'quantity': q,
      'confirmed': _confirmed,
      if (_note.text.trim().isNotEmpty) 'note': _note.text.trim(),
    });
    _length.clear();
    _note.clear();
    _qty.text = '1';
  });

  Future<void> _toggle(Json item, bool confirmed) => _run(() async {
    await widget.controller.api.saveStock(item['id'] as String, {
      'profile': item['profile'],
      'length_mm': item['length_mm'],
      'quantity': item['quantity'],
      'confirmed': confirmed,
      if (item['note'] != null) 'note': item['note'],
    });
  });

  Future<void> _delete(Json item) =>
      _run(() => widget.controller.api.deleteStock(item['id'] as String));

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Stock and offcuts',
                style: Theme.of(context).textTheme.titleLarge,
              ),
            ),
            IconButton(
              onPressed: _reload,
              tooltip: 'Refresh',
              icon: const Icon(Icons.refresh),
            ),
          ],
        ),
        Text(
          'List aluminium you already have. Tick Confirmed only when you have checked it is really there and usable. '
          'Unconfirmed items are never used in a cutting plan. In a new order, use "Load saved stock" to bring this list in.',
          style: Theme.of(context).textTheme.bodySmall,
        ),
        const SizedBox(height: 12),
        Card(
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Wrap(
              spacing: 12,
              runSpacing: 12,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: [
                DropdownButton<String>(
                  value: _profile,
                  items: [
                    for (final e in profileLabels.entries)
                      DropdownMenuItem(value: e.key, child: Text(e.value)),
                  ],
                  onChanged: (v) => setState(() => _profile = v!),
                ),
                SizedBox(
                  width: 150,
                  child: TextField(
                    controller: _length,
                    decoration: const InputDecoration(
                      labelText: 'Length, with unit',
                      hintText: '8 ft',
                      isDense: true,
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                SizedBox(
                  width: 80,
                  child: TextField(
                    controller: _qty,
                    keyboardType: TextInputType.number,
                    decoration: const InputDecoration(
                      labelText: 'Qty',
                      isDense: true,
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                SizedBox(
                  width: 200,
                  child: TextField(
                    controller: _note,
                    decoration: const InputDecoration(
                      labelText: 'Note (optional)',
                      isDense: true,
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Checkbox(
                      value: _confirmed,
                      onChanged: (v) => setState(() => _confirmed = v ?? false),
                    ),
                    const Text('Confirmed'),
                  ],
                ),
                FilledButton.icon(
                  onPressed: _add,
                  icon: const Icon(Icons.add),
                  label: const Text('Add'),
                ),
              ],
            ),
          ),
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              _error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ),
        const SizedBox(height: 8),
        FutureBuilder<List<Json>>(
          future: _items,
          builder: (context, snap) {
            if (snap.connectionState != ConnectionState.done) {
              return const Center(child: CircularProgressIndicator());
            }
            if (snap.hasError) return Text('${snap.error}');
            final items = snap.data!;
            if (items.isEmpty) {
              return const Text(
                'No stock listed. Nothing is assumed to be available.',
              );
            }
            return Column(
              children: [
                for (final it in items)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(
                      '${profileLabels[it['profile']] ?? it['profile']}: ${it['quantity']} x ${fmt(it['length_mm'])} mm',
                    ),
                    subtitle: it['note'] == null ? null : Text('${it['note']}'),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        const Text('Confirmed'),
                        Switch(
                          value: it['confirmed'] == true,
                          onChanged: (v) => _toggle(it, v),
                        ),
                        IconButton(
                          onPressed: () => _delete(it),
                          tooltip: 'Remove',
                          icon: const Icon(Icons.delete_outline),
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

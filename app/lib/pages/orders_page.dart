import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../api.dart';
import '../draft.dart';
import '../format.dart';
import '../report.dart';
import '../theme.dart';
import '../widgets.dart';
import 'result_view.dart';

String _when(Object? iso) {
  final t = DateTime.tryParse('$iso')?.toLocal();
  if (t == null) return '$iso';
  String two(int n) => n.toString().padLeft(2, '0');
  return '${t.year}-${two(t.month)}-${two(t.day)} ${two(t.hour)}:${two(t.minute)}';
}

class OrdersPage extends StatefulWidget {
  const OrdersPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<OrdersPage> createState() => _OrdersPageState();
}

class _OrdersPageState extends State<OrdersPage> {
  late Future<List<Json>> _orders = widget.controller.api.listOrders();
  final _filter = TextEditingController();

  @override
  void dispose() {
    _filter.dispose();
    super.dispose();
  }

  void _reload() => setState(() {
    _orders = widget.controller.api.listOrders();
  });

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        PageHeader(
          'Saved orders',
          subtitle: 'Every calculation is kept; editing adds a new revision',
          actions: [
            MiniButton('Refresh', icon: Icons.refresh, onPressed: _reload),
            FilledButton.icon(
              onPressed: widget.controller.startNewOrder,
              icon: const Icon(Icons.add, size: 18),
              label: const Text('New window order'),
            ),
          ],
        ),
        FutureBuilder<List<Json>>(
          future: _orders,
          builder: (context, snap) {
            if (snap.connectionState != ConnectionState.done) {
              return const Padding(
                padding: EdgeInsets.all(40),
                child: Center(child: CircularProgressIndicator()),
              );
            }
            if (snap.hasError) return Text('${snap.error}');
            final all = snap.data!;
            final q = _filter.text.trim().toLowerCase();
            final orders = q.isEmpty
                ? all
                : all
                      .where(
                        (o) =>
                            '${o['customer_name'] ?? 'no customer name'} ${o['created_by'] ?? ''}'
                                .toLowerCase()
                                .contains(q),
                      )
                      .toList();
            if (all.isEmpty) {
              return const AppCard(
                title: 'Orders',
                collapsible: false,
                child: Text(
                  'No saved orders yet. Calculate an order and press Save order.',
                ),
              );
            }
            const head = TextStyle(fontWeight: FontWeight.w700);
            return AppCard(
              title: q.isEmpty
                  ? 'Orders (${all.length})'
                  : 'Orders (${orders.length} of ${all.length})',
              icon: Icons.folder_open,
              collapsible: false,
              padding: EdgeInsets.zero,
              child: Column(
                children: [
                  Container(
                    color: Colors.white,
                    padding: const EdgeInsets.fromLTRB(14, 10, 14, 10),
                    child: Align(
                      alignment: Alignment.centerLeft,
                      child: SizedBox(
                        width: 320,
                        child: TextField(
                          controller: _filter,
                          onChanged: (_) => setState(() {}),
                          decoration: InputDecoration(
                            hintText: 'Filter by customer or user...',
                            prefixIcon: const Icon(Icons.filter_list, size: 18),
                            suffixIcon: q.isEmpty
                                ? null
                                : IconButton(
                                    icon: const Icon(Icons.close, size: 16),
                                    onPressed: () => setState(() {
                                      _filter.clear();
                                    }),
                                  ),
                          ),
                        ),
                      ),
                    ),
                  ),
                  if (orders.isEmpty)
                    const Padding(
                      padding: EdgeInsets.all(16),
                      child: Text('No orders match that filter.'),
                    ),
                  Container(
                    color: Brand.headBg,
                    padding: const EdgeInsets.symmetric(
                      horizontal: 14,
                      vertical: 9,
                    ),
                    child: const Row(
                      children: [
                        Expanded(flex: 4, child: Text('Customer', style: head)),
                        Expanded(flex: 3, child: Text('Updated', style: head)),
                        Expanded(
                          flex: 2,
                          child: Text('Revisions', style: head),
                        ),
                        Expanded(
                          flex: 2,
                          child: Text('Created by', style: head),
                        ),
                        SizedBox(width: 24),
                      ],
                    ),
                  ),
                  for (final (i, o) in orders.indexed)
                    InkWell(
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute(
                          builder: (_) => OrderDetailPage(
                            controller: widget.controller,
                            orderId: o['id'] as String,
                          ),
                        ),
                      ),
                      child: Container(
                        decoration: BoxDecoration(
                          color: i.isOdd ? const Color(0xFFF8F9FB) : null,
                          border: const Border(
                            bottom: BorderSide(color: Brand.border),
                          ),
                        ),
                        padding: const EdgeInsets.symmetric(
                          horizontal: 14,
                          vertical: 11,
                        ),
                        child: Row(
                          children: [
                            Expanded(
                              flex: 4,
                              child: Text(
                                o['customer_name']?.toString() ??
                                    'No customer name',
                                style: const TextStyle(
                                  color: Brand.blue,
                                  fontWeight: FontWeight.w600,
                                ),
                              ),
                            ),
                            Expanded(
                              flex: 3,
                              child: Text(_when(o['updated_at'])),
                            ),
                            Expanded(
                              flex: 2,
                              child: Text('${o['revision_count']}'),
                            ),
                            Expanded(
                              flex: 2,
                              child: Text(
                                '${o['created_by'] ?? ''}',
                                overflow: TextOverflow.ellipsis,
                              ),
                            ),
                            const Icon(
                              Icons.chevron_right,
                              size: 18,
                              color: Brand.muted,
                            ),
                          ],
                        ),
                      ),
                    ),
                ],
              ),
            );
          },
        ),
      ],
    );
  }
}

class OrderDetailPage extends StatefulWidget {
  const OrderDetailPage({
    super.key,
    required this.controller,
    required this.orderId,
  });
  final AppController controller;
  final String orderId;

  @override
  State<OrderDetailPage> createState() => _OrderDetailPageState();
}

class _OrderDetailPageState extends State<OrderDetailPage> {
  late final Future<Json> _order = widget.controller.api.getOrder(
    widget.orderId,
  );
  bool _pdfBusy = false;

  /// Builds a PDF of this saved calculation (same tables as on screen, with the
  /// letterhead) and asks where to save it.
  Future<void> _downloadPdf(Json o, Json latest, CalcView view) async {
    if (_pdfBusy) return;
    final messenger = ScaffoldMessenger.of(context);
    setState(() {
      _pdfBusy = true;
    });
    try {
      final api = widget.controller.api;
      Json? business;
      Uint8List? logo;
      try {
        business = await api.getBusiness();
        logo = await api.getLogo();
      } on ApiException {
        // the letterhead is optional; the report still prints without it
      }
      final bytes = await buildReportPdf(
        customer: o['customer_name'] as String?,
        revision: latest['revision'] as int,
        calculated: _when(latest['created_at']),
        rulesVersion: latest['rules_version_no'] as int,
        viewLabel: view.label,
        blocks: reportBlocks(view, latest['result'] as Json),
        business: business,
        logo: logo,
      );
      final saved = await FilePicker.saveFile(
        fileName: reportFileName(
          o['customer_name'] as String?,
          latest['revision'] as int,
        ),
        bytes: bytes,
        mimeType: 'application/pdf',
        dialogTitle: 'Save the calculation as PDF',
      );
      if (saved != null) {
        messenger.showSnackBar(SnackBar(content: Text('PDF saved: $saved')));
      }
    } catch (e) {
      messenger.showSnackBar(
        SnackBar(content: Text('Could not make the PDF: $e')),
      );
    } finally {
      if (mounted) {
        setState(() {
          _pdfBusy = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Saved order'), toolbarHeight: 48),
      body: FutureBuilder<Json>(
        future: _order,
        builder: (context, snap) {
          if (snap.connectionState != ConnectionState.done) {
            return const Center(child: CircularProgressIndicator());
          }
          if (snap.hasError) {
            return Padding(
              padding: const EdgeInsets.all(16),
              child: Text('${snap.error}'),
            );
          }
          final o = snap.data!;
          final latest = o['latest'] as Json;
          final request = latest['request'] as Json;
          final view = CalcView.fromWire(request['view'] as String?);
          final revCount = (o['revisions'] as List).length;
          final info = AppCard(
            title: 'Order Information',
            icon: Icons.description_outlined,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                KvGrid([
                  (
                    'Customer',
                    KvGrid.text(
                      o['customer_name']?.toString() ?? 'No customer name',
                      bold: true,
                    ),
                  ),
                  ('Status', const StatusDot('Saved', Brand.ok)),
                  (
                    'Revision',
                    KvGrid.text('${latest['revision']} of $revCount'),
                  ),
                  (
                    'Rules version',
                    KvGrid.text('v${latest['rules_version_no']}'),
                  ),
                  ('Calculated', KvGrid.text(_when(latest['created_at']))),
                  ('Created by', KvGrid.text('${o['created_by'] ?? 'n/a'}')),
                  ('Report view', KvGrid.text(view.label)),
                ]),
                const SizedBox(height: 10),
                Text(
                  'These figures are a snapshot: later price changes do not alter them.',
                  style: Theme.of(context).textTheme.bodySmall,
                ),
                const SizedBox(height: 10),
                Wrap(
                  spacing: 8,
                  runSpacing: 8,
                  children: [
                    MiniButton(
                      _pdfBusy ? 'Making PDF...' : 'Download PDF',
                      icon: Icons.picture_as_pdf_outlined,
                      onPressed: _pdfBusy
                          ? null
                          : () => _downloadPdf(o, latest, view),
                    ),
                    MiniButton(
                      'Edit (new revision)',
                      icon: Icons.edit_outlined,
                      onPressed: () {
                        widget.controller.editOrder(
                          widget.orderId,
                          request,
                          o['customer_name'] as String?,
                        );
                        Navigator.of(context).pop();
                      },
                    ),
                  ],
                ),
              ],
            ),
          );
          final quote = view == CalcView.full
              ? QuotePanel(
                  controller: widget.controller,
                  orderId: widget.orderId,
                  windows: windowLines(request),
                )
              : null;
          final result = AppCard(
            title: '${view.label} view',
            icon: Icons.calculate_outlined,
            child: ResultView(view: view, result: latest['result'] as Json),
          );
          return LayoutBuilder(
            builder: (context, box) {
              final two = box.maxWidth >= 1000 && quote != null;
              return ListView(
                padding: const EdgeInsets.all(20),
                children: [
                  PageHeader(
                    'Order: ${o['customer_name'] ?? 'No customer name'}',
                    subtitle: 'Revision ${latest['revision']} of $revCount',
                    actions: [
                      FilledButton.icon(
                        onPressed: _pdfBusy
                            ? null
                            : () => _downloadPdf(o, latest, view),
                        icon: const Icon(Icons.download, size: 18),
                        label: Text(
                          _pdfBusy ? 'Making PDF...' : 'Download PDF',
                        ),
                      ),
                    ],
                  ),
                  if (two)
                    Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Expanded(flex: 3, child: info),
                        const SizedBox(width: 16),
                        Expanded(flex: 2, child: quote),
                      ],
                    )
                  else ...[
                    info,
                    if (quote != null) ...[const SizedBox(height: 14), quote],
                  ],
                  const SizedBox(height: 14),
                  result,
                ],
              );
            },
          );
        },
      ),
    );
  }
}

/// A window line of a saved order: a name and how many.
class WindowLine {
  const WindowLine(this.name, this.quantity);
  final String name;
  final int quantity;
}

/// The window lines of a saved request, in order (as the server sees them).
List<WindowLine> windowLines(Json request) => [
  for (final w in ((request['windows'] as List?) ?? const []).cast<Map>())
    WindowLine(
      '${w['size'] ?? '${w['width_mm']} x ${w['height_mm']} mm'}'
      '${(w['label'] ?? '').toString().isEmpty ? '' : ' (${w['label']})'}',
      (w['quantity'] as num?)?.toInt() ?? 0,
    ),
];

/// Quotation preview and PDF. Every figure comes from the saved calculation on
/// the server; this widget only chooses the aluminium basis and deposit %.
class QuotePanel extends StatefulWidget {
  const QuotePanel({
    super.key,
    required this.controller,
    required this.orderId,
    this.windows = const [],
  });
  final AppController controller;
  final String orderId;

  /// One entry per window line of the saved order, for unit selling prices.
  final List<WindowLine> windows;

  @override
  State<QuotePanel> createState() => _QuotePanelState();
}

class _QuotePanelState extends State<QuotePanel> {
  static const _bases = {
    'consumption': 'Actual length used',
    'whole_bars': 'Whole 21 ft bars',
    'existing_plus_purchase': 'Existing stock + purchase',
  };
  String _basis = 'consumption';
  final _deposit = TextEditingController();
  final _margin = TextEditingController();
  final _selling = TextEditingController();
  final _terms = TextEditingController();
  late final List<TextEditingController> _unit = [
    for (final _ in widget.windows) TextEditingController(),
  ];
  Json? _quote;
  String? _error;
  bool _busy = false;

  @override
  void dispose() {
    _deposit.dispose();
    _margin.dispose();
    _selling.dispose();
    _terms.dispose();
    for (final c in _unit) {
      c.dispose();
    }
    super.dispose();
  }

  bool get _anyUnit => _unit.any((c) => c.text.trim().isNotEmpty);

  /// Unit prices are all-or-nothing: one price for every window line.
  String? _unitCheck() {
    if (!_anyUnit) return null;
    if (_unit.any((c) => c.text.trim().isEmpty)) {
      return 'Enter a unit price for every window line, or clear them all.';
    }
    return null;
  }

  Map<String, String?> _extra() => {
    'deposit_percent': _deposit.text,
    'margin_percent': _margin.text,
    'selling_price_ksh': _selling.text,
    'unit_prices': _anyUnit
        ? _unit.map((c) => c.text.trim().replaceAll(',', '')).join(',')
        : '',
    'payment_terms': _terms.text,
  };

  Widget _field(TextEditingController c, String label, double width) =>
      SizedBox(
        width: width,
        child: TextField(
          controller: c,
          decoration: InputDecoration(labelText: label, isDense: true),
          onChanged: (_) => _quote = null,
        ),
      );

  Future<void> _preview() async {
    final bad = _unitCheck();
    if (bad != null) {
      setState(() {
        _quote = null;
        _error = bad;
      });
      return;
    }
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final q = await widget.controller.api.getQuote(
        widget.orderId,
        basis: _basis,
        extra: _extra(),
      );
      setState(() => _quote = q);
    } on ApiException catch (e) {
      setState(() {
        _quote = null;
        _error = e.message;
      });
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _openPdf() async {
    final bad = _unitCheck();
    if (bad != null) {
      setState(() => _error = bad);
      return;
    }
    try {
      final uri = await widget.controller.api.quotePdfUri(
        widget.orderId,
        basis: _basis,
        extra: _extra(),
      );
      if (!await launchUrl(uri, mode: LaunchMode.externalApplication) &&
          mounted) {
        setState(() => _error = 'Could not open the PDF in a viewer.');
      }
    } on ApiException catch (e) {
      if (mounted) setState(() => _error = e.message);
    }
  }

  @override
  Widget build(BuildContext context) {
    final q = _quote;
    return AppCard(
      title: 'Quotation',
      icon: Icons.request_quote_outlined,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 12,
            runSpacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              DropdownButton<String>(
                value: _basis,
                items: [
                  for (final e in _bases.entries)
                    DropdownMenuItem(value: e.key, child: Text(e.value)),
                ],
                onChanged: (v) => setState(() {
                  _basis = v!;
                  _quote = null;
                }),
              ),
              _field(_deposit, 'Deposit %', 100),
              _field(_margin, 'Margin % (optional)', 130),
              _field(_selling, 'or selling price KSh', 150),
              _field(_terms, 'Payment terms (optional)', 260),
              OutlinedButton(
                onPressed: _busy ? null : _preview,
                child: const Text('Preview'),
              ),
              FilledButton.icon(
                onPressed: _openPdf,
                icon: const Icon(Icons.picture_as_pdf),
                label: const Text('Open PDF'),
              ),
            ],
          ),
          if (widget.windows.isNotEmpty) ...[
            const SizedBox(height: 12),
            Text(
              'Unit selling price for each window (optional)',
              style: Theme.of(context).textTheme.titleSmall,
            ),
            const SizedBox(height: 2),
            Text(
              'Type the price you will charge per window. The system multiplies by the quantity and adds them up. Use this instead of a margin or a selling price, not as well.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
            const SizedBox(height: 8),
            Wrap(
              spacing: 12,
              runSpacing: 10,
              children: [
                for (final (i, w) in widget.windows.indexed)
                  _field(
                    _unit[i],
                    '${w.name} x ${w.quantity}: price each (KSh)',
                    260,
                  ),
              ],
            ),
          ],
          if (_error != null)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(
                _error!,
                style: TextStyle(color: Theme.of(context).colorScheme.error),
              ),
            ),
          if (q != null) ...[
            const SizedBox(height: 8),
            for (final l in (q['lines'] as List).cast<Json>())
              Row(
                children: [
                  Expanded(child: Text('${l['description']}')),
                  Text(ksh(l['amount_ksh']?.toString())),
                ],
              ),
            const Divider(),
            if (q['total_low_ksh'] == q['total_high_ksh'])
              Row(
                children: [
                  const Expanded(
                    child: Text(
                      'TOTAL',
                      style: TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ),
                  Text(
                    ksh(q['total_low_ksh']?.toString()),
                    style: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                ],
              )
            else
              Row(
                children: [
                  const Expanded(
                    child: Text(
                      'TOTAL (range)',
                      style: TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ),
                  Text(
                    '${ksh(q['total_low_ksh']?.toString())} to ${ksh(q['total_high_ksh']?.toString())}',
                    style: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                ],
              ),
            if (q['selling'] != null) ...[
              const Divider(),
              if (q['priced_windows'] != null)
                for (final w in (q['priced_windows'] as List).cast<Json>())
                  Row(
                    children: [
                      Expanded(
                        child: Text(
                          '${w['description']}: ${w['quantity']} x ${ksh(w['unit_price_ksh'])}',
                        ),
                      ),
                      Text(ksh(w['line_total_ksh'])),
                    ],
                  ),
              Row(
                children: [
                  const Expanded(
                    child: Text(
                      'Selling price (customer pays)',
                      style: TextStyle(fontWeight: FontWeight.bold),
                    ),
                  ),
                  Text(
                    ksh(q['selling']['selling_price_ksh']),
                    style: const TextStyle(fontWeight: FontWeight.bold),
                  ),
                ],
              ),
              Row(
                children: [
                  const Expanded(
                    child: Text('Your profit (private, not on the PDF)'),
                  ),
                  Text(
                    '${ksh(q['selling']['profit_ksh'])}  (${fmt(q['selling']['margin_percent'])}% margin)',
                  ),
                ],
              ),
            ],
            if (q['deposit_ksh'] != null) ...[
              Row(
                children: [
                  Expanded(child: Text('Deposit (${q['deposit_percent']}%)')),
                  Text(ksh(q['deposit_ksh']?.toString())),
                ],
              ),
              Row(
                children: [
                  const Expanded(child: Text('Balance')),
                  Text(ksh(q['balance_ksh']?.toString())),
                ],
              ),
            ],
            if (q['confirmed'] != true)
              const Text(
                'UNCONFIRMED: estimate only, not a final price.',
                style: TextStyle(fontWeight: FontWeight.bold),
              ),
            if (q['transport_note'] != null) Text('${q['transport_note']}'),
            for (final n in (q['notes'] as List)) Text('$n'),
          ],
        ],
      ),
    );
  }
}

import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';
import '../theme.dart';
import '../widgets.dart';
import 'orders_page.dart';

/// Start screen: what is saved, what is in stock, which prices are in force.
/// Counts come straight from the server; nothing is calculated here.
class DashboardPage extends StatefulWidget {
  const DashboardPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<DashboardPage> createState() => _DashboardPageState();
}

class _Data {
  _Data(this.orders, this.stock, this.rules);
  final List<Json> orders;
  final List<Json> stock;
  final Json rules;
}

class _DashboardPageState extends State<DashboardPage> {
  late Future<_Data> _data = _load();

  Future<_Data> _load() async {
    final a = widget.controller.api;
    final r = await Future.wait([
      a.listOrders(),
      a.listStock(),
      a.currentRules(),
    ]);
    return _Data(r[0] as List<Json>, r[1] as List<Json>, r[2] as Json);
  }

  static String _when(Object? iso) {
    final t = DateTime.tryParse('$iso')?.toLocal();
    if (t == null) return '$iso';
    String two(int n) => n.toString().padLeft(2, '0');
    return '${t.year}-${two(t.month)}-${two(t.day)} ${two(t.hour)}:${two(t.minute)}';
  }

  @override
  Widget build(BuildContext context) {
    final c = widget.controller;
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        PageHeader(
          'Dashboard',
          subtitle: 'Saved work, stock and the prices in force',
          actions: [
            MiniButton(
              'Refresh',
              icon: Icons.refresh,
              onPressed: () => setState(() {
                _data = _load();
              }),
            ),
            FilledButton.icon(
              onPressed: c.startNewOrder,
              icon: const Icon(Icons.add, size: 18),
              label: const Text('New window order'),
            ),
          ],
        ),
        FutureBuilder<_Data>(
          future: _data,
          builder: (context, snap) {
            if (snap.connectionState != ConnectionState.done) {
              return const Padding(
                padding: EdgeInsets.all(40),
                child: Center(child: CircularProgressIndicator()),
              );
            }
            if (snap.hasError) return Text('${snap.error}');
            final d = snap.data!;
            final confirmed = d.stock
                .where((s) => s['confirmed'] == true)
                .length;
            final unconfirmed = d.stock.length - confirmed;
            final kpis = [
              KpiCard(
                icon: Icons.folder_open,
                value: '${d.orders.length}',
                label: 'Saved orders',
                tone: Brand.blue,
                onTap: () => c.goTo(AppController.tabOrders),
              ),
              KpiCard(
                icon: Icons.inventory_2_outlined,
                value: '${d.stock.length}',
                label: 'Stock items',
                sub: '$confirmed confirmed',
                tone: Brand.navy,
                onTap: () => c.goTo(AppController.tabStock),
              ),
              KpiCard(
                icon: unconfirmed == 0
                    ? Icons.check_circle_outline
                    : Icons.help_outline,
                value: '$unconfirmed',
                label: 'Stock to confirm',
                sub: unconfirmed == 0 ? 'All confirmed' : 'Needs checking',
                tone: unconfirmed == 0 ? Brand.ok : Brand.warn,
                onTap: () => c.goTo(AppController.tabStock),
              ),
              KpiCard(
                icon: Icons.sell_outlined,
                value: 'v${d.rules['version_no']}',
                label: 'Prices and rules in force',
                tone: Brand.ok,
                onTap: () => c.goTo(AppController.tabSettings),
              ),
            ];
            return Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                LayoutBuilder(
                  builder: (context, box) {
                    final cols = box.maxWidth >= 1000
                        ? 4
                        : (box.maxWidth >= 560 ? 2 : 1);
                    final w = (box.maxWidth - 12 * (cols - 1)) / cols;
                    return Wrap(
                      spacing: 12,
                      runSpacing: 12,
                      children: [
                        for (final k in kpis) SizedBox(width: w, child: k),
                      ],
                    );
                  },
                ),
                const SizedBox(height: 16),
                AppCard(
                  title: 'Status at a glance',
                  icon: Icons.speed_outlined,
                  child: Wrap(
                    spacing: 40,
                    runSpacing: 16,
                    crossAxisAlignment: WrapCrossAlignment.center,
                    children: [
                      RingGauge(
                        percent: d.stock.isEmpty
                            ? null
                            : (confirmed * 100 / d.stock.length).round(),
                        label: 'Stock confirmed %',
                      ),
                      RingGauge(
                        percent: d.orders.isEmpty
                            ? null
                            : (d.orders
                                          .where(
                                            (o) =>
                                                (o['revision_count'] as num) >
                                                1,
                                          )
                                          .length *
                                      100 /
                                      d.orders.length)
                                  .round(),
                        label: 'Orders revised %',
                      ),
                      ConstrainedBox(
                        constraints: const BoxConstraints(
                          minWidth: 260,
                          maxWidth: 420,
                        ),
                        child: KvGrid([
                          (
                            'Saved orders',
                            KvGrid.text('${d.orders.length}', bold: true),
                          ),
                          (
                            'Stock items',
                            KvGrid.text('${d.stock.length}', bold: true),
                          ),
                          (
                            'Rules version',
                            KvGrid.text(
                              'v${d.rules['version_no']}',
                              bold: true,
                            ),
                          ),
                          ('Signed in', KvGrid.text(c.userLabel)),
                        ]),
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                AppCard(
                  title: 'Recent orders',
                  icon: Icons.history,
                  padding: EdgeInsets.zero,
                  trailing: Padding(
                    padding: const EdgeInsets.only(right: 8),
                    child: TextButton(
                      onPressed: () => c.goTo(AppController.tabOrders),
                      child: const Text('View all'),
                    ),
                  ),
                  child: d.orders.isEmpty
                      ? const Padding(
                          padding: EdgeInsets.all(16),
                          child: Text('No saved orders yet.'),
                        )
                      : Column(
                          children: [
                            Container(
                              color: Brand.headBg,
                              padding: const EdgeInsets.symmetric(
                                horizontal: 14,
                                vertical: 8,
                              ),
                              child: const Row(
                                children: [
                                  Expanded(
                                    flex: 4,
                                    child: Text(
                                      'Customer',
                                      style: TextStyle(
                                        fontWeight: FontWeight.w700,
                                      ),
                                    ),
                                  ),
                                  Expanded(
                                    flex: 3,
                                    child: Text(
                                      'Updated',
                                      style: TextStyle(
                                        fontWeight: FontWeight.w700,
                                      ),
                                    ),
                                  ),
                                  Expanded(
                                    flex: 2,
                                    child: Text(
                                      'Revisions',
                                      style: TextStyle(
                                        fontWeight: FontWeight.w700,
                                      ),
                                    ),
                                  ),
                                  Expanded(
                                    flex: 2,
                                    child: Text(
                                      'By',
                                      style: TextStyle(
                                        fontWeight: FontWeight.w700,
                                      ),
                                    ),
                                  ),
                                ],
                              ),
                            ),
                            for (final (i, o) in d.orders.take(6).indexed)
                              InkWell(
                                onTap: () => Navigator.of(context).push(
                                  MaterialPageRoute(
                                    builder: (_) => OrderDetailPage(
                                      controller: c,
                                      orderId: o['id'] as String,
                                    ),
                                  ),
                                ),
                                child: Container(
                                  color: i.isOdd
                                      ? const Color(0xFFF8F9FB)
                                      : null,
                                  padding: const EdgeInsets.symmetric(
                                    horizontal: 14,
                                    vertical: 10,
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
                                    ],
                                  ),
                                ),
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

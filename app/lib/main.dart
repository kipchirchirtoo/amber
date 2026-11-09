import 'package:flutter/material.dart';

import 'draft.dart';
import 'pages/assistant_page.dart';
import 'pages/dashboard_page.dart';
import 'pages/login_page.dart';
import 'pages/new_order.dart';
import 'pages/orders_page.dart';
import 'pages/settings_page.dart';
import 'pages/stock_page.dart';
import 'theme.dart';
import 'topbar.dart';
import 'pages/profile_page.dart';

void main() => runApp(const AmberApp());

class AmberApp extends StatefulWidget {
  const AmberApp({super.key, this.controller});
  final AppController? controller;

  @override
  State<AmberApp> createState() => _AmberAppState();
}

class _AmberAppState extends State<AmberApp> {
  // Created once and kept. (Making it inside build() would start a brand new,
  // signed-out controller every time this widget rebuilt, e.g. on hot reload.)
  late final AppController _controller = widget.controller ?? AppController();

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Amber Stores',
      debugShowCheckedModeBanner: false,
      theme: buildTheme(),
      home: Gate(controller: _controller),
    );
  }
}

/// Login screen until someone is signed in, then the app.
class Gate extends StatelessWidget {
  const Gate({super.key, required this.controller});
  final AppController controller;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: controller,
    builder: (context, _) => controller.user == null
        ? LoginPage(controller: controller)
        : Shell(controller: controller),
  );
}

class Shell extends StatefulWidget {
  const Shell({super.key, required this.controller});
  final AppController controller;

  @override
  State<Shell> createState() => _ShellState();
}

class _ShellState extends State<Shell> {
  bool _collapsed = false;

  // (tab index, outlined icon, filled icon, label). Grouped like an ERP menu.
  static const _groups = [
    (
      'OVERVIEW',
      [
        (
          AppController.tabDashboard,
          Icons.dashboard_outlined,
          Icons.dashboard,
          'Dashboard',
        ),
      ],
    ),
    (
      'SALES',
      [
        (
          AppController.tabNew,
          Icons.add_box_outlined,
          Icons.add_box,
          'New order',
        ),
        (
          AppController.tabAssistant,
          Icons.chat_bubble_outline,
          Icons.chat_bubble,
          'Assistant',
        ),
        (
          AppController.tabOrders,
          Icons.folder_open_outlined,
          Icons.folder_open,
          'Saved orders',
        ),
      ],
    ),
    (
      'INVENTORY',
      [
        (
          AppController.tabStock,
          Icons.inventory_2_outlined,
          Icons.inventory_2,
          'Stock',
        ),
      ],
    ),
    (
      'SETUP',
      [
        (
          AppController.tabSettings,
          Icons.settings_outlined,
          Icons.settings,
          'Settings',
        ),
      ],
    ),
  ];

  Widget _sidebar(AppController c) {
    return Container(
      width: _collapsed ? 64 : 208,
      decoration: const BoxDecoration(
        color: Colors.white,
        border: Border(right: BorderSide(color: Brand.border)),
      ),
      child: Column(
        children: [
          Expanded(
            child: ListView(
              padding: const EdgeInsets.symmetric(vertical: 8),
              children: [
                for (final g in _groups) ...[
                  if (!_collapsed)
                    Padding(
                      padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
                      child: Text(
                        g.$1,
                        style: const TextStyle(
                          fontSize: 11,
                          fontWeight: FontWeight.w700,
                          color: Brand.muted,
                          letterSpacing: 0.8,
                        ),
                      ),
                    )
                  else
                    const Divider(height: 16),
                  for (final d in g.$2) _navItem(c, d),
                ],
              ],
            ),
          ),
          const Divider(height: 1),
          InkWell(
            onTap: () => setState(() {
              _collapsed = !_collapsed;
            }),
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Row(
                mainAxisAlignment: _collapsed
                    ? MainAxisAlignment.center
                    : MainAxisAlignment.start,
                children: [
                  Icon(
                    _collapsed
                        ? Icons.keyboard_double_arrow_right
                        : Icons.keyboard_double_arrow_left,
                    size: 18,
                    color: Brand.muted,
                  ),
                  if (!_collapsed)
                    const Padding(
                      padding: EdgeInsets.only(left: 10),
                      child: Text(
                        'Collapse',
                        style: TextStyle(color: Brand.muted),
                      ),
                    ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _navItem(AppController c, (int, IconData, IconData, String) d) {
    final sel = c.tab == d.$1;
    return Tooltip(
      message: _collapsed ? d.$4 : '',
      child: InkWell(
        onTap: () => c.goTo(d.$1),
        child: Container(
          margin: const EdgeInsets.symmetric(horizontal: 8, vertical: 1),
          padding: EdgeInsets.symmetric(
            horizontal: _collapsed ? 0 : 10,
            vertical: 9,
          ),
          decoration: BoxDecoration(
            color: sel ? const Color(0xFFE3EDFD) : null,
            borderRadius: BorderRadius.circular(6),
          ),
          child: Row(
            mainAxisAlignment: _collapsed
                ? MainAxisAlignment.center
                : MainAxisAlignment.start,
            children: [
              Icon(
                sel ? d.$3 : d.$2,
                size: 20,
                color: sel ? Brand.blue : Brand.muted,
              ),
              if (!_collapsed) ...[
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    d.$4,
                    overflow: TextOverflow.ellipsis,
                    style: TextStyle(
                      fontWeight: sel ? FontWeight.w700 : FontWeight.w500,
                      color: sel ? Brand.blue : Brand.text,
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        // The new-order page stays alive so a half-entered order is never lost
        // when the user glances at saved orders.
        // Screens other than New order are rebuilt each time they are opened so
        // they always show current server data.
        Widget fresh(int i, Widget Function() make) =>
            controller.tab == i ? make() : const SizedBox.shrink();
        final body = IndexedStack(
          index: controller.tab,
          children: [
            fresh(
              AppController.tabDashboard,
              () => DashboardPage(key: UniqueKey(), controller: controller),
            ),
            NewOrderPage(controller: controller),
            AssistantPage(
              controller: controller,
            ), // stays alive so the chat is not lost
            fresh(
              AppController.tabOrders,
              () => OrdersPage(key: UniqueKey(), controller: controller),
            ),
            fresh(
              AppController.tabStock,
              () => StockPage(key: UniqueKey(), controller: controller),
            ),
            fresh(
              AppController.tabSettings,
              () => SettingsPage(key: UniqueKey(), controller: controller),
            ),
            fresh(
              AppController.tabProfile,
              () => ProfilePage(controller: controller),
            ),
          ],
        );
        final wide = MediaQuery.sizeOf(context).width >= 800;
        final flat = [for (final g in _groups) ...g.$2];
        return Scaffold(
          appBar: TopBar(controller: controller, wide: wide),
          body: wide
              ? Row(
                  children: [
                    _sidebar(controller),
                    Expanded(child: body),
                  ],
                )
              : body,
          bottomNavigationBar: wide
              ? null
              : NavigationBar(
                  selectedIndex: flat
                      .indexWhere((d) => d.$1 == controller.tab)
                      .clamp(0, flat.length - 1),
                  onDestinationSelected: (i) => controller.goTo(flat[i].$1),
                  destinations: [
                    for (final d in flat)
                      NavigationDestination(
                        icon: Icon(d.$2),
                        selectedIcon: Icon(d.$3),
                        label: d.$4,
                      ),
                  ],
                ),
        );
      },
    );
  }
}

import 'package:flutter/material.dart';

import 'api.dart';
import 'draft.dart';
import 'format.dart';
import 'pages/orders_page.dart';
import 'theme.dart';
import 'widgets.dart';

/// Navy header: brand on the left, a search box that finds orders, stock and
/// screens in the middle, and the signed-in person's avatar menu on the right.
class TopBar extends StatelessWidget implements PreferredSizeWidget {
  const TopBar({super.key, required this.controller, required this.wide});
  final AppController controller;
  final bool wide;

  @override
  Size get preferredSize => const Size.fromHeight(56);

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Brand.navy,
      child: SafeArea(
        bottom: false,
        child: SizedBox(
          height: 56,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16),
            child: Row(
              children: [
                Container(
                  padding: const EdgeInsets.all(5),
                  decoration: BoxDecoration(
                    color: Colors.white.withValues(alpha: 0.14),
                    borderRadius: BorderRadius.circular(6),
                  ),
                  child: const Icon(
                    Icons.window_outlined,
                    size: 20,
                    color: Colors.white,
                  ),
                ),
                const SizedBox(width: 10),
                if (wide) ...[
                  const Text(
                    'Amber Stores Kenya',
                    style: TextStyle(
                      color: Colors.white,
                      fontSize: 16,
                      fontWeight: FontWeight.w700,
                    ),
                  ),
                  const SizedBox(width: 10),
                  const Text(
                    'Sliding Window Calculator',
                    style: TextStyle(fontSize: 12, color: Color(0xFFB9C7DB)),
                  ),
                ],
                Expanded(
                  child: Center(
                    child: ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 520),
                      child: Padding(
                        padding: EdgeInsets.symmetric(
                          horizontal: wide ? 24 : 8,
                        ),
                        child: GlobalSearch(controller: controller),
                      ),
                    ),
                  ),
                ),
                _AvatarMenu(controller: controller, wide: wide),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _AvatarMenu extends StatelessWidget {
  const _AvatarMenu({required this.controller, required this.wide});
  final AppController controller;
  final bool wide;

  @override
  Widget build(BuildContext context) {
    final name = controller.userLabel;
    final role = controller.isAdmin ? 'Administrator' : 'Staff';
    return PopupMenuButton<String>(
      tooltip: 'Account',
      offset: const Offset(0, 46),
      onSelected: (v) {
        if (v == 'profile') controller.goTo(AppController.tabProfile);
        if (v == 'signout') controller.signOut();
      },
      itemBuilder: (_) => [
        PopupMenuItem(
          enabled: false,
          child: Row(
            children: [
              UserAvatar(name, radius: 18),
              const SizedBox(width: 10),
              Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    name,
                    style: const TextStyle(
                      fontWeight: FontWeight.w700,
                      color: Brand.text,
                    ),
                  ),
                  Text(
                    role,
                    style: const TextStyle(fontSize: 12, color: Brand.muted),
                  ),
                ],
              ),
            ],
          ),
        ),
        const PopupMenuDivider(),
        const PopupMenuItem(
          value: 'profile',
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.person_outline),
            title: Text('Profile settings'),
          ),
        ),
        const PopupMenuItem(
          value: 'signout',
          child: ListTile(
            dense: true,
            contentPadding: EdgeInsets.zero,
            leading: Icon(Icons.logout),
            title: Text('Sign out'),
          ),
        ),
      ],
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 8),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            UserAvatar(name),
            if (wide) ...[
              const SizedBox(width: 10),
              Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    name,
                    style: const TextStyle(
                      color: Colors.white,
                      fontSize: 13,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  Text(
                    role,
                    style: const TextStyle(
                      color: Color(0xFFB9C7DB),
                      fontSize: 11,
                    ),
                  ),
                ],
              ),
            ],
            const Icon(Icons.arrow_drop_down, color: Colors.white),
          ],
        ),
      ),
    );
  }
}

class _Hit {
  _Hit(this.icon, this.title, this.subtitle, this.onTap, this.haystack);
  final IconData icon;
  final String title;
  final String? subtitle;
  final VoidCallback onTap;
  final String haystack;
}

/// One search box for everything: screens and actions, saved orders (by
/// customer), and stock items. It only looks things up; nothing is calculated.
class GlobalSearch extends StatefulWidget {
  const GlobalSearch({super.key, required this.controller});
  final AppController controller;

  @override
  State<GlobalSearch> createState() => _GlobalSearchState();
}

class _GlobalSearchState extends State<GlobalSearch> {
  List<Json> _orders = const [];
  List<Json> _stock = const [];
  DateTime? _loadedAt;

  AppController get c => widget.controller;

  Future<void> _refreshData() async {
    final at = _loadedAt;
    if (at != null &&
        DateTime.now().difference(at) < const Duration(seconds: 20)) {
      return;
    }
    _loadedAt = DateTime.now();
    try {
      final r = await Future.wait([c.api.listOrders(), c.api.listStock()]);
      _orders = r[0];
      _stock = r[1];
    } on ApiException {
      // searching still works for screens when the lists cannot be loaded
    }
  }

  List<_Hit> _pages() => [
    _Hit(
      Icons.dashboard_outlined,
      'Dashboard',
      'Screen',
      () => c.goTo(AppController.tabDashboard),
      'dashboard home overview summary',
    ),
    _Hit(
      Icons.add_box_outlined,
      'New window order',
      'Action',
      c.startNewOrder,
      'new order window calculate quote create',
    ),
    _Hit(
      Icons.chat_bubble_outline,
      'Assistant',
      'Screen',
      () => c.goTo(AppController.tabAssistant),
      'assistant ai chat ask',
    ),
    _Hit(
      Icons.folder_open_outlined,
      'Saved orders',
      'Screen',
      () => c.goTo(AppController.tabOrders),
      'saved orders history quotations',
    ),
    _Hit(
      Icons.inventory_2_outlined,
      'Stock',
      'Screen',
      () => c.goTo(AppController.tabStock),
      'stock inventory offcuts bars',
    ),
    _Hit(
      Icons.person_outline,
      'Profile settings',
      'Screen',
      () => c.goTo(AppController.tabProfile),
      'profile account password change my',
    ),
    _Hit(
      Icons.settings_outlined,
      'Settings',
      'Screen',
      () => c.goTo(AppController.tabSettings),
      'settings server address rules prices',
    ),
    _Hit(
      Icons.sell_outlined,
      'Prices and rules',
      'Settings',
      () => c.goTo(AppController.tabSettings),
      'prices rates rubber glass labour fittings formulas history',
    ),
    _Hit(
      Icons.receipt_long_outlined,
      'Quotation letterhead and payment details',
      'Settings',
      () => c.goTo(AppController.tabSettings),
      'letterhead logo payment bank mpesa business details quotation',
    ),
    if (c.isAdmin)
      _Hit(
        Icons.group_outlined,
        'Users',
        'Settings (administrator)',
        () => c.goTo(AppController.tabSettings),
        'users staff accounts people add',
      ),
    _Hit(
      Icons.logout,
      'Sign out',
      'Action',
      c.signOut,
      'sign out log out logout',
    ),
  ];

  List<_Hit> _orderHits(BuildContext context) => [
    for (final o in _orders)
      _Hit(
        Icons.folder_open,
        o['customer_name']?.toString() ?? 'No customer name',
        'Order, ${o['revision_count']} revision${o['revision_count'] == 1 ? '' : 's'}',
        () => Navigator.of(context).push(
          MaterialPageRoute(
            builder: (_) =>
                OrderDetailPage(controller: c, orderId: o['id'] as String),
          ),
        ),
        '${o['customer_name'] ?? 'no customer name'} ${o['created_by'] ?? ''} ${o['id']}',
      ),
  ];

  List<_Hit> _stockHits() => [
    for (final it in _stock)
      _Hit(
        Icons.inventory_2_outlined,
        '${profileLabels[it['profile']] ?? it['profile']}: ${it['quantity']} x ${fmt(it['length_mm'])} mm',
        it['note'] == null ? 'Stock item' : 'Stock item, ${it['note']}',
        () => c.goTo(AppController.tabStock),
        '${profileLabels[it['profile']] ?? it['profile']} ${it['profile']} ${it['length_mm']} ${it['quantity']} ${it['note'] ?? ''} stock',
      ),
  ];

  @override
  Widget build(BuildContext context) {
    return SearchAnchor(
      isFullScreen: false,
      viewConstraints: const BoxConstraints(maxHeight: 420),
      viewBackgroundColor: Colors.white,
      builder: (context, search) => SizedBox(
        height: 38,
        child: SearchBar(
          controller: search,
          hintText: 'Search orders, stock, screens...',
          hintStyle: const WidgetStatePropertyAll(
            TextStyle(color: Brand.muted, fontSize: 13),
          ),
          textStyle: const WidgetStatePropertyAll(
            TextStyle(color: Brand.text, fontSize: 13),
          ),
          elevation: const WidgetStatePropertyAll(0),
          backgroundColor: const WidgetStatePropertyAll(Colors.white),
          shape: WidgetStatePropertyAll(
            RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
          ),
          padding: const WidgetStatePropertyAll(
            EdgeInsets.symmetric(horizontal: 12),
          ),
          leading: const Icon(Icons.search, size: 20, color: Brand.muted),
          onTap: () {
            _refreshData();
            search.openView();
          },
          onChanged: (_) => search.openView(),
        ),
      ),
      suggestionsBuilder: (context, search) async {
        await _refreshData();
        final q = search.text.trim().toLowerCase();
        final words = q
            .split(RegExp(r'\s+'))
            .where((w) => w.isNotEmpty)
            .toList();
        bool match(_Hit h) => words.every(
          (w) =>
              h.haystack.toLowerCase().contains(w) ||
              h.title.toLowerCase().contains(w),
        );
        final pages = _pages().where(match).toList();
        if (!context.mounted) {
          return <Widget>[];
        }
        final orders = _orderHits(context).where(match).toList();
        final stock = _stockHits().where(match).toList();
        if (words.isNotEmpty &&
            pages.isEmpty &&
            orders.isEmpty &&
            stock.isEmpty) {
          return [
            Padding(
              padding: const EdgeInsets.all(16),
              child: Text('Nothing found for "${search.text.trim()}".'),
            ),
          ];
        }
        Widget header(String s) => Padding(
          padding: const EdgeInsets.fromLTRB(16, 10, 16, 4),
          child: Text(
            s,
            style: const TextStyle(
              fontSize: 11,
              fontWeight: FontWeight.w700,
              color: Brand.muted,
              letterSpacing: 0.8,
            ),
          ),
        );
        Widget tile(_Hit h) => ListTile(
          dense: true,
          leading: Icon(h.icon, size: 20, color: Brand.navy),
          title: Text(h.title),
          subtitle: h.subtitle == null ? null : Text(h.subtitle!),
          onTap: () {
            search.closeView(null);
            search.clear();
            h.onTap();
          },
        );
        return [
          if (orders.isNotEmpty) ...[
            header('SAVED ORDERS'),
            for (final h in orders.take(6)) tile(h),
          ],
          if (stock.isNotEmpty) ...[
            header('STOCK'),
            for (final h in stock.take(6)) tile(h),
          ],
          if (pages.isNotEmpty) ...[
            header(words.isEmpty ? 'GO TO' : 'SCREENS AND ACTIONS'),
            for (final h in pages.take(8)) tile(h),
          ],
        ];
      },
    );
  }
}

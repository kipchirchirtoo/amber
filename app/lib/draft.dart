import 'package:flutter/widgets.dart';

import 'api.dart';

/// What the user can ask to see (matches the server's `view` values).
enum CalcView {
  standard('Default', 'standard'),
  aluminium('Aluminium', 'aluminium'),
  rubber('Rubber', 'rubber'),
  rubberRolls('Rubber, full rolls', 'rubber_rolls'),
  stock('Bars to buy', 'stock'),
  panels('Glass panels', 'panels'),
  fittings('Fittings', 'fittings'),
  labour('Labour', 'labour'),
  transport('Transport', 'transport'),
  glassSheets('Glass sheets', 'glass_sheets'),
  full('Full costing', 'full');

  const CalcView(this.label, this.wire);
  final String label;
  final String wire;

  static CalcView fromWire(String? w) => CalcView.values.firstWhere(
    (v) => v.wire == w,
    orElse: () => CalcView.standard,
  );
}

class WindowRow {
  WindowRow({String size = '', String qty = '1', String label = ''})
    : size = TextEditingController(text: size),
      qty = TextEditingController(text: qty),
      label = TextEditingController(text: label);

  final TextEditingController size;
  final TextEditingController qty;
  final TextEditingController label;

  bool get isBlank => size.text.trim().isEmpty;

  void dispose() {
    size.dispose();
    qty.dispose();
    label.dispose();
  }
}

class StockRow {
  StockRow({
    this.profile = 'side_track',
    String length = '',
    String qty = '1',
    this.confirmed = false,
  }) : length = TextEditingController(text: length),
       qty = TextEditingController(text: qty);

  String profile;
  final TextEditingController length;
  final TextEditingController qty;
  bool confirmed;

  void dispose() {
    length.dispose();
    qty.dispose();
  }
}

/// The order being entered. Only collects what the user typed; the server and
/// engine interpret and calculate it.
class OrderDraft extends ChangeNotifier {
  final customer = TextEditingController();
  final kerf = TextEditingController();
  final transport = TextEditingController();
  String? defaultUnit; // null = every size must carry its own unit
  bool includeGlass = false;
  final windows = <WindowRow>[WindowRow()];
  final stock = <StockRow>[];

  void addWindow() {
    windows.add(WindowRow());
    notifyListeners();
  }

  void removeWindow(WindowRow r) {
    windows.remove(r);
    r.dispose();
    if (windows.isEmpty) windows.add(WindowRow());
    notifyListeners();
  }

  /// Adds a row from a saved stock item. The length is sent back to the server
  /// as as its mm figure, the exact figure it stored, so nothing is converted here.
  void addStockFrom(Json item) {
    stock.add(
      StockRow(
        profile: item['profile'] as String,
        length: '${item['length_mm']} mm',
        qty: '${item['quantity']}',
        confirmed: item['confirmed'] == true,
      ),
    );
    notifyListeners();
  }

  void addStock() {
    stock.add(StockRow());
    notifyListeners();
  }

  void removeStock(StockRow r) {
    stock.remove(r);
    r.dispose();
    notifyListeners();
  }

  void touch() => notifyListeners();

  void clear() {
    customer.clear();
    kerf.clear();
    transport.clear();
    defaultUnit = null;
    includeGlass = false;
    for (final w in windows) {
      w.dispose();
    }
    for (final s in stock) {
      s.dispose();
    }
    windows
      ..clear()
      ..add(WindowRow());
    stock.clear();
    notifyListeners();
  }

  /// Fill the form from a saved request so an order can be reviewed or edited.
  void loadRequest(Json req, {String? customerName}) {
    clear();
    customer.text = customerName ?? '';
    defaultUnit = req['default_unit'] as String?;
    windows.clear();
    for (final w in (req['windows'] as List? ?? const [])) {
      final m = w as Map;
      final size =
          m['size']?.toString() ?? '${m['width_mm']} x ${m['height_mm']} mm';
      windows.add(
        WindowRow(
          size: size,
          qty: '${m['quantity']}',
          label: m['label']?.toString() ?? '',
        ),
      );
    }
    if (windows.isEmpty) windows.add(WindowRow());
    final o = (req['options'] as Map?) ?? const {};
    final k = o['kerf_mm']?.toString() ?? '';
    kerf.text = (k == '0') ? '' : k;
    transport.text = o['transport_exact_ksh']?.toString() ?? '';
    includeGlass = o['include_glass_sheets'] == true;
    for (final s in (o['existing_stock'] as List? ?? const [])) {
      final m = s as Map;
      stock.add(
        StockRow(
          profile: m['profile'].toString(),
          length: m['length']?.toString() ?? '${m['length_mm']} mm',
          qty: '${m['quantity']}',
          confirmed: m['confirmed'] == true,
        ),
      );
    }
    notifyListeners();
  }

  /// Build the API request, or throw [ApiException] with a plain message.
  Json toRequest(CalcView view) {
    final rows = windows.where((w) => !w.isBlank).toList();
    if (rows.isEmpty) {
      throw ApiException('Enter at least one window size, e.g. "4 x 4 ft".');
    }
    final ws = <Json>[];
    for (var i = 0; i < rows.length; i++) {
      final q = int.tryParse(rows[i].qty.text.trim());
      if (q == null || q < 1) {
        throw ApiException(
          'Window ${i + 1}: quantity must be a whole number of 1 or more.',
        );
      }
      ws.add({
        'size': rows[i].size.text.trim(),
        'quantity': q,
        if (rows[i].label.text.trim().isNotEmpty)
          'label': rows[i].label.text.trim(),
      });
    }
    final options = <String, dynamic>{};
    if (kerf.text.trim().isNotEmpty) options['kerf_mm'] = kerf.text.trim();
    if (transport.text.trim().isNotEmpty) {
      options['transport_exact_ksh'] = transport.text.trim();
    }
    if (includeGlass) options['include_glass_sheets'] = true;
    final st = <Json>[];
    for (var i = 0; i < stock.length; i++) {
      final s = stock[i];
      if (s.length.text.trim().isEmpty) continue;
      final q = int.tryParse(s.qty.text.trim());
      if (q == null || q < 1) {
        throw ApiException(
          'Existing stock ${i + 1}: quantity must be a whole number of 1 or more.',
        );
      }
      st.add({
        'profile': s.profile,
        'length': s.length.text.trim(),
        'quantity': q,
        'confirmed': s.confirmed,
      });
    }
    if (st.isNotEmpty) options['existing_stock'] = st;
    return {
      'windows': ws,
      if (defaultUnit != null) 'default_unit': defaultUnit,
      'view': view.wire,
      if (options.isNotEmpty) 'options': options,
    };
  }

  String? get customerName =>
      customer.text.trim().isEmpty ? null : customer.text.trim();
}

/// App-wide state: server connection, the draft, and which saved order is being edited.
class AppController extends ChangeNotifier {
  AppController({ApiClient? api}) : api = api ?? ApiClient() {
    this.api.onUnauthorized = _sessionEnded;
  }

  final ApiClient api;
  final draft = OrderDraft();
  String? editingOrderId;

  /// The signed-in user ({id, username, display_name, role}); null when signed out.
  Json? user;

  /// Why the login screen is showing (e.g. the session expired).
  String? signedOutReason;

  bool get isAdmin => user?['role'] == 'admin';
  String get userLabel => '${user?['display_name'] ?? user?['username'] ?? ''}';

  Future<void> signIn(String server, String username, String password) async {
    api.baseUrl = server;
    user = await api.login(username, password);
    signedOutReason = null;
    tab = tabDashboard;
    notifyListeners();
  }

  Future<void> signOut() async {
    await api.logout();
    _clearSession(null);
  }

  void _sessionEnded() {
    if (user != null) {
      _clearSession('You were signed out. Please sign in again.');
    }
  }

  void _clearSession(String? reason) {
    user = null;
    api.token = null;
    signedOutReason = reason;
    editingOrderId = null;
    draft.clear();
    notifyListeners();
  }

  static const tabDashboard = 0,
      tabNew = 1,
      tabAssistant = 2,
      tabOrders = 3,
      tabStock = 4,
      tabSettings = 5,
      tabProfile = 6;
  int tab = tabDashboard;

  void goTo(int i) {
    tab = i;
    notifyListeners();
  }

  void startNewOrder() {
    editingOrderId = null;
    draft.clear();
    goTo(tabNew);
  }

  void editOrder(String id, Json request, String? customer) {
    editingOrderId = id;
    draft.loadRequest(request, customerName: customer);
    goTo(tabNew);
  }
}

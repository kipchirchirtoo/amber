import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:http/http.dart' as http;

class ApiException implements Exception {
  ApiException(this.message, {this.unauthorized = false});
  final String message;

  /// The server said the login is missing or no longer valid.
  final bool unauthorized;
  @override
  String toString() => message;
}

typedef Json = Map<String, dynamic>;

/// Thin client for amber_server. No calculation happens here or anywhere in
/// the app: requests go out, engine results come back.
class ApiClient {
  /// On the web the app is served by the same host as the API (see deploy/), so it
  /// talks to its own address. Elsewhere it starts at the local development server.
  ApiClient({String? baseUrl, http.Client? client})
    : baseUrl = baseUrl ?? _defaultBaseUrl(),
      _client = client ?? http.Client();

  /// Where the server is, unless the user types another address on the sign-in
  /// screen. Set when building: `--dart-define=API_URL=https://your-server`.
  /// Otherwise the web app talks to its own host and the others to localhost.
  static const _builtInUrl = String.fromEnvironment('API_URL');
  static String _defaultBaseUrl() {
    if (_builtInUrl.isNotEmpty) return _builtInUrl;
    return kIsWeb ? Uri.base.origin : 'http://localhost:8080';
  }

  String baseUrl;

  /// Login token. Kept in memory only: closing the app signs you out.
  String? token;

  /// Called when the server rejects the token (expired, signed out elsewhere, user disabled).
  void Function()? onUnauthorized;

  final http.Client _client;

  Uri _uri(String path, [Map<String, String>? q]) => Uri.parse(
    baseUrl.replaceAll(RegExp(r'/+$'), '') + path,
  ).replace(queryParameters: q);

  Future<http.Response> _do(
    String method,
    String path, {
    Object? body,
    Uint8List? bytes,
    String? contentType,
    Map<String, String>? query,
    Duration timeout = const Duration(seconds: 60),
  }) async {
    final req = http.Request(method, _uri(path, query));
    if (token != null) req.headers['authorization'] = 'Bearer $token';
    if (bytes != null) {
      req.headers['content-type'] = contentType ?? 'application/octet-stream';
      req.bodyBytes = bytes;
    } else {
      req.headers['content-type'] = 'application/json';
      if (body != null) req.body = jsonEncode(body);
    }
    try {
      return await http.Response.fromStream(
        await _client.send(req).timeout(timeout),
      );
    } catch (e) {
      throw ApiException(
        'Cannot reach the server at $baseUrl. Is it running? ($e)',
      );
    }
  }

  Never _fail(http.Response res, dynamic data, {bool isLogin = false}) {
    final msg = data is Map && data['error'] is Map
        ? data['error']['message']
        : null;
    final expired = res.statusCode == 401 && !isLogin;
    if (expired) onUnauthorized?.call();
    throw ApiException(
      msg?.toString() ?? 'Server error (${res.statusCode})',
      unauthorized: expired,
    );
  }

  Future<dynamic> _send(
    String method,
    String path, {
    Object? body,
    Map<String, String>? query,
    Duration timeout = const Duration(seconds: 60),
    bool isLogin = false,
  }) async {
    final res = await _do(
      method,
      path,
      body: body,
      query: query,
      timeout: timeout,
    );
    dynamic data;
    try {
      data = res.body.isEmpty ? null : jsonDecode(utf8.decode(res.bodyBytes));
    } catch (_) {
      data = null;
    }
    if (res.statusCode >= 200 && res.statusCode < 300) return data;
    _fail(res, data, isLogin: isLogin);
  }

  // ---- signing in

  /// Signs in and remembers the token. Returns the user.
  Future<Json> login(String username, String password) async {
    final r =
        await _send(
              'POST',
              '/api/v1/auth/login',
              body: {'username': username, 'password': password},
              isLogin: true,
            )
            as Json;
    token = r['token'] as String;
    return r['user'] as Json;
  }

  Future<void> logout() async {
    try {
      await _send('POST', '/api/v1/auth/logout');
    } on ApiException {
      // signing out must always work locally
    }
    token = null;
  }

  /// Changes your own password; every other device is signed out and this one gets a new token.
  Future<void> changePassword(String current, String next) async {
    final r =
        await _send(
              'POST',
              '/api/v1/auth/password',
              body: {'current_password': current, 'new_password': next},
            )
            as Json;
    token = r['token'] as String;
  }

  Future<List<Json>> listUsers() async =>
      ((await _send('GET', '/api/v1/users')) as List).cast<Json>();

  Future<Json> createUser({
    required String username,
    required String displayName,
    required String role,
    required String password,
  }) async =>
      (await _send(
            'POST',
            '/api/v1/users',
            body: {
              'username': username,
              'display_name': displayName,
              'role': role,
              'password': password,
            },
          ))
          as Json;

  Future<Json> updateUser(String id, Json changes) async =>
      (await _send('PUT', '/api/v1/users/$id', body: changes)) as Json;

  // ---- calculating and orders

  Future<Json> calculate(Json request) async =>
      (await _send('POST', '/api/v1/calculate', body: request)) as Json;

  Future<Json> createOrder(String? customer, Json request) async =>
      (await _send(
            'POST',
            '/api/v1/orders',
            body: {'customer_name': customer, 'request': request},
          ))
          as Json;

  Future<Json> updateOrder(String id, String? customer, Json request) async =>
      (await _send(
            'PUT',
            '/api/v1/orders/$id',
            body: {'customer_name': customer, 'request': request},
          ))
          as Json;

  Future<List<Json>> listOrders() async =>
      ((await _send('GET', '/api/v1/orders', query: {'limit': '100'})) as List)
          .cast<Json>();

  static Map<String, String> _quoteParams(
    String basis,
    Map<String, String?> extra,
  ) => {
    'basis': basis,
    for (final e in extra.entries)
      if (e.value != null && e.value!.trim().isNotEmpty) e.key: e.value!.trim(),
  };

  Future<String> _ticket() async =>
      ((await _send('POST', '/api/v1/auth/ticket')) as Json)['ticket']
          as String;

  /// Address of the quotation PDF, good for two minutes, for opening in the
  /// browser or a PDF viewer (a browser tab cannot send the login token).
  /// [extra] keys: deposit_percent, margin_percent, selling_price_ksh, payment_terms.
  Future<Uri> quotePdfUri(
    String id, {
    String basis = 'consumption',
    Map<String, String?> extra = const {},
  }) async => _uri('/api/v1/orders/$id/quote.pdf', {
    ..._quoteParams(basis, extra),
    'ticket': await _ticket(),
  });

  /// Same, for a ready-made server path such as the assistant's quote_pdf_path.
  Future<Uri> ticketedUri(String pathAndQuery) async {
    final u = Uri.parse(baseUrl.replaceAll(RegExp(r'/+$'), '') + pathAndQuery);
    return u.replace(
      queryParameters: {...u.queryParameters, 'ticket': await _ticket()},
    );
  }

  Future<Json> getQuote(
    String id, {
    String basis = 'consumption',
    Map<String, String?> extra = const {},
  }) async =>
      (await _send(
            'GET',
            '/api/v1/orders/$id/quote',
            query: _quoteParams(basis, extra),
          ))
          as Json;

  Future<Json> getOrder(String id) async =>
      (await _send('GET', '/api/v1/orders/$id')) as Json;

  // ---- prices and business profile

  Future<Json> currentRules() async =>
      (await _send('GET', '/api/v1/settings/rules')) as Json;

  Future<List<Json>> rulesHistory() async =>
      ((await _send('GET', '/api/v1/settings/rules/history')) as List)
          .cast<Json>();

  /// Appends a new rules version (administrators only). The server keeps every
  /// earlier version and refuses formula changes unless authorised.
  Future<Json> updateRules(
    Json rules,
    String note, {
    bool authoriseFormulaChange = false,
  }) async =>
      (await _send(
            'PUT',
            '/api/v1/settings/rules',
            body: {
              'rules': rules,
              'note': note,
              'authorise_formula_change': authoriseFormulaChange,
            },
          ))
          as Json;

  Future<Json> getBusiness() async =>
      (await _send('GET', '/api/v1/settings/business')) as Json;

  Future<Json> saveBusiness(Json profile) async =>
      (await _send('PUT', '/api/v1/settings/business', body: profile)) as Json;

  /// The letterhead logo, or null if none is set.
  Future<Uint8List?> getLogo() async {
    final res = await _do('GET', '/api/v1/settings/business/logo');
    if (res.statusCode == 404) return null;
    if (res.statusCode >= 200 && res.statusCode < 300) return res.bodyBytes;
    _fail(res, null);
  }

  Future<void> uploadLogo(Uint8List jpeg) async {
    final res = await _do(
      'PUT',
      '/api/v1/settings/business/logo',
      bytes: jpeg,
      contentType: 'image/jpeg',
    );
    if (res.statusCode >= 200 && res.statusCode < 300) return;
    dynamic data;
    try {
      data = jsonDecode(utf8.decode(res.bodyBytes));
    } catch (_) {}
    _fail(res, data);
  }

  Future<void> deleteLogo() async =>
      _send('DELETE', '/api/v1/settings/business/logo');

  // ---- stock

  Future<List<Json>> listStock() async =>
      ((await _send('GET', '/api/v1/stock')) as List).cast<Json>();

  /// [id] null creates a new item; otherwise replaces that item.
  Future<Json> saveStock(String? id, Json body) async =>
      (await _send(
            id == null ? 'POST' : 'PUT',
            id == null ? '/api/v1/stock' : '/api/v1/stock/$id',
            body: body,
          ))
          as Json;

  Future<void> deleteStock(String id) async =>
      _send('DELETE', '/api/v1/stock/$id');

  // ---- assistant

  Future<bool> assistantEnabled() async =>
      ((await _send('GET', '/api/v1/assistant/status')) as Json)['enabled'] ==
      true;

  /// The model can take a while to answer, so this waits longer than other calls.
  Future<Json> assistantChat(String? sessionId, String message) async =>
      (await _send(
            'POST',
            '/api/v1/assistant/chat',
            body: {'session_id': ?sessionId, 'message': message},
            timeout: const Duration(seconds: 120),
          ))
          as Json;

  Future<bool> health() async {
    await _send('GET', '/health');
    return true;
  }
}

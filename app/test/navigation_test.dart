import 'dart:convert';
import 'dart:io';

import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:amber_app/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

http.Response _json(Object body) => http.Response(
  jsonEncode(body),
  200,
  headers: {'content-type': 'application/json'},
);

void main() {
  testWidgets('going back from a saved order stays signed in on Saved orders', (
    t,
  ) async {
    t.view.physicalSize = const Size(1600, 1000);
    t.view.devicePixelRatio = 1;
    addTearDown(t.view.reset);

    final result =
        jsonDecode(File('test/fixtures/full_brief.json').readAsStringSync())
            as Json;
    final order = {
      'id': 'o1',
      'customer_name': 'KIPCHIRCHIR TOO',
      'revision_count': 1,
      'updated_at': '2026-11-08T16:55:00Z',
      'created_by': 'admin@amber.com',
    };
    final client = MockClient((req) async {
      final p = req.url.path;
      if (p == '/api/v1/auth/login') {
        return _json({
          'token': 'tok',
          'user': {
            'id': 'u1',
            'username': 'admin@amber.com',
            'display_name': 'Administrator',
            'role': 'admin',
          },
        });
      }
      if (p == '/api/v1/orders') return _json([order]);
      if (p == '/api/v1/orders/o1') {
        return _json({
          ...order,
          'revisions': [1],
          'latest': {
            'revision': 1,
            'created_at': '2026-11-08T16:55:00Z',
            'rules_version_no': 1,
            'request': {'view': 'full'},
            'result': result,
          },
        });
      }
      if (p == '/api/v1/stock') return _json(<Object>[]);
      if (p == '/api/v1/settings/rules') return _json({'version_no': 1});
      return _json({});
    });
    final c = AppController(api: ApiClient(client: client));
    await t.pumpWidget(AmberApp(controller: c));
    await c.signIn('http://x', 'admin@amber.com', 'pw');
    await t.pumpAndSettle();

    c.goTo(AppController.tabOrders);
    await t.pumpAndSettle();
    await t.tap(find.text('KIPCHIRCHIR TOO'));
    await t.pumpAndSettle();
    expect(find.text('Saved order'), findsOneWidget);

    await t.tap(find.byType(BackButton));
    await t.pumpAndSettle();

    expect(find.text('Sign in'), findsNothing, reason: 'was sent to login');
    expect(c.user, isNotNull);
    expect(find.text('Saved orders'), findsWidgets);
  });
}

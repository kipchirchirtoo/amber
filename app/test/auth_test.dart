import 'dart:convert';

import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:amber_app/main.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

http.Response _json(int status, Object body) => http.Response(
  jsonEncode(body),
  status,
  headers: {'content-type': 'application/json'},
);

void main() {
  test(
    'sign-in keeps the token in memory and sends it; a 401 signs the user out',
    () async {
      String? seenAuth;
      var expired = false;
      final client = MockClient((req) async {
        if (req.url.path == '/api/v1/auth/login') {
          final b = jsonDecode(req.body) as Map;
          if (b['password'] != 'right password') {
            return _json(401, {
              'error': {
                'code': 'bad_login',
                'message': 'wrong username or password',
              },
            });
          }
          return _json(200, {
            'token': 'tok123',
            'user': {
              'id': 'u1',
              'username': 'amina',
              'display_name': 'Amina',
              'role': 'staff',
            },
          });
        }
        seenAuth = req.headers['authorization'];
        if (expired) {
          return _json(401, {
            'error': {'code': 'unauthorized', 'message': 'please sign in'},
          });
        }
        return _json(200, <Object>[]);
      });
      final c = AppController(api: ApiClient(client: client));

      await expectLater(
        c.signIn('http://x', 'amina', 'nope'),
        throwsA(
          isA<ApiException>().having(
            (e) => e.message,
            'message',
            'wrong username or password',
          ),
        ),
      );
      expect(c.user, isNull);

      await c.signIn('http://x', 'amina', 'right password');
      expect(c.user?['username'], 'amina');
      expect(c.isAdmin, false);
      await c.api.listOrders();
      expect(seenAuth, 'Bearer tok123');

      expired = true; // e.g. an administrator switched the account off
      await expectLater(
        c.api.listOrders(),
        throwsA(
          isA<ApiException>().having(
            (e) => e.unauthorized,
            'unauthorized',
            true,
          ),
        ),
      );
      expect(c.user, isNull);
      expect(c.api.token, isNull);
      expect(c.signedOutReason, contains('signed out'));
    },
  );

  testWidgets('the login screen shows until someone signs in', (t) async {
    final c = AppController(
      api: ApiClient(client: MockClient((_) async => _json(200, <Object>[]))),
    );
    await t.pumpWidget(MaterialApp(home: Gate(controller: c)));
    expect(find.text('Sign in'), findsOneWidget);
    expect(find.text('Username'), findsOneWidget);
    expect(find.text('Dashboard'), findsNothing);
  });
}

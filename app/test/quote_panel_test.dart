import 'dart:convert';

import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:amber_app/pages/orders_page.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

void main() {
  test('window lines come from the saved request in order', () {
    final lines = windowLines({
      'windows': [
        {'size': '4 x 4 ft', 'quantity': 5, 'label': 'lounge'},
        {'width_mm': '600', 'height_mm': '900', 'quantity': 2},
      ],
    });
    expect(lines.map((l) => l.name), ['4 x 4 ft (lounge)', '600 x 900 mm']);
    expect(lines.map((l) => l.quantity), [5, 2]);
  });

  testWidgets('unit prices are all-or-nothing and are sent in window order', (
    t,
  ) async {
    t.view.physicalSize = const Size(1400, 2400);
    t.view.devicePixelRatio = 1;
    addTearDown(t.view.reset);
    final seen = <Uri>[];
    final client = MockClient((req) async {
      seen.add(req.url);
      return http.Response(
        jsonEncode({
          'lines': [],
          'total_low_ksh': '100',
          'total_high_ksh': '100',
          'confirmed': true,
          'notes': [],
          'selling': {
            'cost_ksh': '100',
            'selling_price_ksh': '49000',
            'profit_ksh': '1',
            'margin_percent': '1.97',
          },
          'priced_windows': [
            {
              'description': '4 x 4 ft',
              'quantity': 5,
              'unit_price_ksh': '5000',
              'line_total_ksh': '25000',
            },
          ],
        }),
        200,
        headers: {'content-type': 'application/json'},
      );
    });
    final c = AppController(api: ApiClient(client: client));
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: QuotePanel(
              controller: c,
              orderId: 'o1',
              windows: const [
                WindowLine('4 x 4 ft', 5),
                WindowLine('2 x 2 ft', 2),
              ],
            ),
          ),
        ),
      ),
    );

    // only one of two prices typed: refused on the screen, nothing is sent
    await t.enterText(
      find.widgetWithText(TextField, '4 x 4 ft x 5: price each (KSh)'),
      '5000',
    );
    await t.tap(find.text('Preview'));
    await t.pumpAndSettle();
    expect(find.textContaining('every window line'), findsOneWidget);
    expect(seen, isEmpty);

    // both typed: sent in window order, and the priced lines are shown
    await t.enterText(
      find.widgetWithText(TextField, '2 x 2 ft x 2: price each (KSh)'),
      '1,500',
    );
    await t.tap(find.text('Preview'));
    await t.pumpAndSettle();
    expect(seen, hasLength(1));
    expect(seen.single.queryParameters['unit_prices'], '5000,1500');
    expect(find.textContaining('4 x 4 ft: 5 x'), findsOneWidget);
  });
}

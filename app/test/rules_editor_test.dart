import 'dart:convert';

import 'package:amber_app/api.dart';
import 'package:amber_app/pages/rules_editor.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;
import 'package:http/testing.dart';

Json rules() => {
  'version': 'amber-brief-1.0',
  'stock_length_mm': '6400.8',
  'stock_price_ksh': '2000',
  'cuts': [
    {
      'profile': 'top_track',
      'basis': 'width',
      'adjust_mm': '-20',
      'pieces_per_window': 1,
    },
    {
      'profile': 'sash',
      'basis': 'half_width',
      'adjust_mm': '20',
      'pieces_per_window': 4,
    },
  ],
  'panels': {
    'panels_per_window': 2,
    'width_deduction_mm': '75',
    'height_deduction_mm': '110',
  },
  'rubber': {
    'rate_per_metre_ksh': '32.80',
    'roll_metres': '38',
    'roll_price_ksh': '1250',
  },
  'fittings': [
    {'kind': 'roller', 'per_window': 4, 'unit_price_ksh': '45'},
  ],
  'labour': {
    'small_max_ft': '2',
    'small_rate_ksh': '500',
    'large_rate_ksh': '700',
  },
  'transport': [
    {'max_windows': 6, 'min_ksh': '300', 'max_ksh': '500'},
    {'max_windows': null, 'min_ksh': '700', 'max_ksh': '1000'},
  ],
  'glass': {'sheet_price_ksh': '6000', 'billing_area_sqft': '38', 'yields': []},
};

void main() {
  Future<List<Map>> pump(WidgetTester t, List<Map> puts) async {
    t.view.physicalSize = const Size(1400, 2400);
    t.view.devicePixelRatio = 1;
    addTearDown(t.view.reset);
    final client = MockClient((req) async {
      puts.add({'method': req.method, 'body': jsonDecode(req.body)});
      return http.Response(
        jsonEncode({'version_no': 2}),
        200,
        headers: {'content-type': 'application/json'},
      );
    });
    await t.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(
            child: RulesEditor(
              api: ApiClient(client: client),
              rules: rules(),
              onSaved: () {},
            ),
          ),
        ),
      ),
    );
    return puts;
  }

  testWidgets(
    'a formula change is refused until the administrator authorises it',
    (t) async {
      final puts = <Map>[];
      await pump(t, puts);
      await t.tap(find.text('Measurement formulas (fixed business rules)'));
      await t.pumpAndSettle();

      await t.enterText(
        find.widgetWithText(TextField, 'Top track: width + (mm)'),
        '-25',
      );
      await t.enterText(
        find.widgetWithText(TextField, 'Reason for change'),
        'trial',
      );
      await t.tap(find.text('Save new version'));
      await t.pumpAndSettle();

      expect(find.textContaining('Tick "I authorise'), findsOneWidget);
      expect(
        puts,
        isEmpty,
        reason: 'nothing may be sent without authorisation',
      );
    },
  );

  testWidgets(
    'an authorised formula change asks to confirm, then is sent as authorised',
    (t) async {
      final puts = <Map>[];
      await pump(t, puts);
      await t.tap(find.text('Measurement formulas (fixed business rules)'));
      await t.pumpAndSettle();
      await t.enterText(
        find.widgetWithText(TextField, 'Top track: width + (mm)'),
        '-25',
      );
      await t.tap(find.text('I authorise changing the measurement formulas'));
      await t.enterText(
        find.widgetWithText(TextField, 'Reason for change'),
        'client approved',
      );
      await t.tap(find.text('Save new version'));
      await t.pumpAndSettle();

      expect(find.text('Change the measurement formulas?'), findsOneWidget);
      expect(find.textContaining('-20  →  -25'), findsOneWidget);
      expect(puts, isEmpty, reason: 'nothing is sent before the confirmation');

      await t.tap(find.text('Yes, I authorise this change'));
      await t.pumpAndSettle();

      expect(puts, hasLength(1));
      final body = puts.single['body'] as Map;
      expect(body['authorise_formula_change'], isTrue);
      expect((body['rules']['cuts'] as List).first['adjust_mm'], '-25');
    },
  );

  testWidgets('a price-only change never claims to be a formula change', (
    t,
  ) async {
    final puts = <Map>[];
    await pump(t, puts);
    await t.enterText(
      find.widgetWithText(TextField, 'Aluminium bar price (KSh)'),
      '2100',
    );
    await t.enterText(
      find.widgetWithText(TextField, 'Reason for change'),
      'supplier increase',
    );
    await t.tap(find.text('Save new version'));
    await t.pumpAndSettle();

    expect(puts, hasLength(1));
    final body = puts.single['body'] as Map;
    expect(body['authorise_formula_change'], isFalse);
    expect(body['rules']['stock_price_ksh'], '2100');
    expect((body['rules']['cuts'] as List).first['adjust_mm'], '-20');
  });
}

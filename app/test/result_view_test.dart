import 'dart:convert';
import 'dart:io';

import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:amber_app/pages/result_view.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

/// full_brief.json is the engine's own output for the brief's example order
/// (5 x 4x4 ft, 3 x 5x4 ft, 2 x 2x2 ft), copied from the engine golden tests.
Json fixture() =>
    jsonDecode(File('test/fixtures/full_brief.json').readAsStringSync())
        as Json;

Future<void> pump(WidgetTester t, CalcView v, Json r) async {
  t.view.physicalSize = const Size(1000, 6000);
  t.view.devicePixelRatio = 1;
  addTearDown(t.view.reset);
  await t.pumpWidget(
    MaterialApp(
      home: Scaffold(
        body: SingleChildScrollView(
          child: ResultView(view: v, result: r),
        ),
      ),
    ),
  );
}

void main() {
  testWidgets('default view shows aluminium table and rubber only', (t) async {
    final f = fixture();
    await pump(t, CalcView.standard, {
      'aluminium': f['aluminium'],
      'rubber': f['rubber'],
    });
    expect(find.text('Combined aluminium requirements'), findsOneWidget);
    expect(find.text('Glass-panel rubber requirements'), findsOneWidget);
    expect(find.text('Top track'), findsOneWidget);
    expect(find.text('112,585.60'), findsOneWidget); // grand total mm
    expect(find.text('369.38'), findsOneWidget); // grand total ft
    expect(find.text('60.27 m'), findsOneWidget);
    expect(find.text('KSh 32.80'), findsOneWidget);
    expect(find.text('KSh 1,976.71'), findsOneWidget);
    // nothing else is shown unless requested
    expect(find.textContaining('Fittings'), findsNothing);
    expect(find.textContaining('abour'), findsNothing);
    expect(find.textContaining('Transport'), findsNothing);
  });

  testWidgets(
    'full costing shows every section and flags unconfirmed transport',
    (t) async {
      await pump(t, CalcView.full, fixture());
      for (final s in [
        'Combined aluminium requirements',
        'Glass panels',
        'Option A',
        'Fittings and accessories',
        'Fundi labour',
        'Transport (once per order)',
        'Project totals',
      ]) {
        expect(find.textContaining(s), findsWidgets, reason: s);
      }
      expect(find.textContaining('UNCONFIRMED'), findsWidgets);
      expect(find.text('KSh 6,600.00'), findsOneWidget); // labour total
      expect(find.text('KSh 3,480.00'), findsOneWidget); // fittings total
      expect(find.text('KSh 40,000.00'), findsWidgets); // 20 bars x 2000
      // glass sheets were not requested, so they are absent
      expect(find.textContaining('Glass sheets'), findsNothing);
    },
  );

  testWidgets('unconfirmed cutting plan is shown clearly', (t) async {
    final plan = {
      'status': 'unconfirmed',
      'kerf_mm': '0',
      'total_new_bars': 1,
      'full_bar_purchase_cost_ksh': '2000',
      'profiles': [
        {
          'profile': 'side_track',
          'status': 'unconfirmed',
          'issues': [
            {'kind': 'cut_longer_than_stock', 'length_mm': '7000', 'count': 1},
          ],
          'new_bars_to_buy': 1,
          'full_bar_purchase_cost_ksh': '2000',
          'existing_pieces_used': 0,
          'lower_bound_bars_without_existing': 1,
          'bins': [],
          'existing_available_mm': '0',
          'ignored_unconfirmed_pieces': 0,
          'shortage_length_mm': '7000',
          'shortage_length_ft': '22.97',
          'shortage_cost_proportional_ksh': '2187',
        },
      ],
    };
    await pump(t, CalcView.stock, plan);
    expect(
      find.textContaining('UNCONFIRMED: cutting plan could not be verified'),
      findsOneWidget,
    );
    expect(
      find.textContaining('7,000.00 mm are longer than a stock bar'),
      findsOneWidget,
    );
  });

  testWidgets('glass sheets needing the fundi show no invented quantity', (
    t,
  ) async {
    await pump(t, CalcView.glassSheets, {
      'status': 'needs_fundi_confirmation',
      'total_sheets': null,
      'billed_area_sqft': null,
      'cost_ksh': null,
      'lines': [
        {
          'rounded_width_ft': 2,
          'rounded_height_ft': 2,
          'windows': 3,
          'status': 'needs_fundi_confirmation',
          'sheets': null,
          'bonus_2x2_windows_not_applied': 0,
        },
      ],
    });
    expect(find.text('ask fundi'), findsOneWidget);
    expect(find.textContaining('No total'), findsOneWidget);
  });
}

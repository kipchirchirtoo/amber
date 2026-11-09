import 'dart:convert';
import 'dart:io';

import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:amber_app/report.dart';
import 'package:flutter_test/flutter_test.dart';

Json fixture() =>
    jsonDecode(File('test/fixtures/full_brief.json').readAsStringSync())
        as Json;

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  test(
    'every view of a saved result becomes report blocks with the engine figures',
    () {
      final r = fixture();
      final blocks = reportBlocks(CalcView.full, r);
      final titles = blocks.map((b) => b.title).toList();
      expect(titles, contains('Combined aluminium requirements'));
      expect(titles, contains('Glass-panel rubber requirements'));
      expect(titles, contains('Fundi labour'));
      final alu = blocks.firstWhere(
        (b) => b.title == 'Combined aluminium requirements',
      );
      expect(alu.rows.last.first, 'Grand total');
      expect(alu.boldLast, isTrue);
    },
  );

  test('the default view report is the aluminium table plus rubber only', () {
    final r = fixture();
    final blocks = reportBlocks(CalcView.standard, {
      'aluminium': r['aluminium'],
      'rubber': r['rubber'],
    });
    expect(blocks.map((b) => b.title), [
      'Combined aluminium requirements',
      'Glass-panel rubber requirements',
    ]);
  });

  test(
    'a PDF is produced for the full calculation, with and without a letterhead',
    () async {
      final r = fixture();
      final blocks = reportBlocks(CalcView.full, r);
      for (final business in [
        null,
        <String, dynamic>{
          'company_name': 'Amber Stores Kenya',
          'address': 'Industrial Area\nNairobi',
          'phone': '0700 000 000',
          'email': 'info@example.com',
          'website': '',
          'tax_pin': 'P000000000X',
        },
      ]) {
        final bytes = await buildReportPdf(
          customer: 'Kipchirchir Too',
          revision: 1,
          calculated: '2026-11-08 16:55',
          rulesVersion: 1,
          viewLabel: 'Full costing',
          blocks: blocks,
          business: business,
        );
        final head = String.fromCharCodes(bytes.take(5));
        final tail = String.fromCharCodes(bytes.skip(bytes.length - 8));
        expect(head, '%PDF-');
        expect(tail, contains('%%EOF'));
        expect(bytes.length, greaterThan(5000));
      }
    },
  );

  test('file names are safe', () {
    expect(
      reportFileName('Kipchirchir Too', 2),
      'calculation-kipchirchir-too-rev2.pdf',
    );
    expect(reportFileName(null, 1), 'calculation-order-rev1.pdf');
    expect(reportFileName('../../x:y*', 3), 'calculation-x-y-rev3.pdf');
  });
}

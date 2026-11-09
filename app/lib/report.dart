import 'dart:typed_data';

import 'package:flutter/services.dart' show rootBundle;
import 'package:pdf/pdf.dart';
import 'package:pdf/widgets.dart' as pw;

import 'api.dart';
import 'draft.dart';
import 'format.dart';

/// One titled table (plus optional notes) of an engine result. The screen and
/// the PDF both read these, so they always show the same figures. Every value
/// is formatted from a string the engine produced; nothing is calculated here.
class ReportBlock {
  const ReportBlock(
    this.title, {
    this.headers = const [],
    this.rows = const [],
    this.boldLast = false,
    this.notes = const [],
    this.status,
  });
  final String title;
  final List<String> headers;
  final List<List<String>> rows;
  final bool boldLast;
  final List<String> notes;

  /// A one-line status shown above the table, e.g. "Cutting plan verified".
  final String? status;
}

String issueText(Json i) => switch (i['kind']) {
  'cut_longer_than_stock' =>
    '${i['count']} cut(s) of ${fmt(i['length_mm'])} mm are longer than a stock bar',
  'verification_failed' => 'Verification failed: ${i['detail']}',
  _ => '$i',
};

const glassStatusText = {
  'confirmed': 'Confirmed by fundi yields',
  'approximate': 'Approximate (yield is approximate)',
  'upper_bound_mixed_sizes':
      'Upper bound: sharing sheets between sizes is unconfirmed',
  'needs_fundi_confirmation': 'Needs fundi confirmation',
};

class Blocks {
  static ReportBlock aluminium(Json r) => ReportBlock(
    'Combined aluminium requirements',
    headers: const [
      'Profile',
      'Total pieces',
      'Total length (mm)',
      'Total length (ft)',
    ],
    rows: [
      for (final p in (r['profiles'] as List).cast<Json>())
        [
          profileLabels[p['profile']] ?? '${p['profile']}',
          '${p['total_pieces']}',
          fmt(p['total_length_mm']),
          fmt(p['total_length_ft']),
        ],
      [
        'Grand total',
        '${r['grand_total_pieces']}',
        fmt(r['grand_total_length_mm']),
        fmt(r['grand_total_length_ft']),
      ],
    ],
    boldLast: true,
  );

  static ReportBlock rubber(Json r) => ReportBlock(
    'Glass-panel rubber requirements',
    headers: const ['Item', 'Value'],
    rows: [
      ['Total rubber required', '${fmt(r['total_metres'], dp: 2)} m'],
      ['Rate per metre', ksh(r['rate_per_metre_ksh'])],
      ['Total rubber consumption cost', ksh(r['consumption_cost_ksh'])],
    ],
  );

  static ReportBlock rubberRolls(Json r) {
    final c = r['consumption'] as Json;
    return ReportBlock(
      'Rubber bought as whole rolls',
      headers: const ['Item', 'Value'],
      rows: [
        ['Rubber needed (actual metres)', '${fmt(c['total_metres'], dp: 2)} m'],
        ['Roll length', '${fmt(r['roll_metres'], dp: 2)} m'],
        ['Roll price', ksh(r['roll_price_ksh'])],
        ['Whole rolls to buy', '${r['rolls']}'],
        ['Left over on the last roll', '${fmt(r['unused_metres'], dp: 2)} m'],
        ['Full-roll purchase cost', ksh(r['full_roll_cost_ksh'])],
      ],
      boldLast: true,
      notes: [
        'Shown only because full-roll purchasing was requested. Normal costing charges the actual metres: ${ksh(c['consumption_cost_ksh'])} at ${ksh(c['rate_per_metre_ksh'])} per metre.',
      ],
    );
  }

  static ReportBlock panels(Json r) => ReportBlock(
    'Glass panels (actual cutting sizes, ${r['total_panels']} panels)',
    headers: const ['Width (mm)', 'Height (mm)', 'Panels'],
    rows: [
      for (final g in (r['groups'] as List).cast<Json>())
        [
          fmt(g['width_mm'], dp: 1, group: false),
          fmt(g['height_mm'], dp: 1, group: false),
          '${g['count']}',
        ],
    ],
  );

  static ReportBlock stock(String title, Json plan) {
    final verified = plan['status'] == 'verified';
    final profiles = (plan['profiles'] as List).cast<Json>();
    final notes = <String>[
      '*Minimum by length is total length divided by bar length. It is only a floor: the bars to buy come from the physical cutting plan.',
      for (final p in profiles)
        for (final i in (p['issues'] as List).cast<Json>())
          '${profileLabels[p['profile']]}: ${issueText(i)}',
      for (final p in profiles) ...[
        '${profileLabels[p['profile']]} cutting plan:',
        for (final (n, b) in (p['bins'] as List).cast<Json>().indexed)
          '  ${b['source'] == 'new' ? 'New bar' : 'Existing ${fmt(b['capacity_mm'])} mm'} ${n + 1}: '
              '${(b['cuts_mm'] as List).map((c) => fmt(c, group: false)).join(' + ')} mm '
              '(left over ${fmt(b['remaining_mm'])} mm)',
        if (p['existing_available_mm'] != '0')
          '  Confirmed existing stock ${fmt(p['existing_available_mm'])} mm; length-based shortage ${fmt(p['shortage_length_mm'])} mm '
              '(${fmt(p['shortage_length_ft'])} ft, about ${ksh(p['shortage_cost_proportional_ksh'])} at the per-foot rate; indicative only).',
        if ((p['ignored_unconfirmed_pieces'] as num) > 0)
          '  ${p['ignored_unconfirmed_pieces']} unconfirmed existing piece(s) were ignored.',
      ],
    ];
    return ReportBlock(
      title,
      status: verified
          ? 'Cutting plan verified'
          : 'UNCONFIRMED: cutting plan could not be verified',
      headers: const [
        'Profile',
        'Bars to buy',
        'Existing pieces used',
        'Minimum by length*',
        'Cost (whole bars)',
        'Status',
      ],
      rows: [
        for (final p in profiles)
          [
            profileLabels[p['profile']] ?? '${p['profile']}',
            '${p['new_bars_to_buy']}',
            '${p['existing_pieces_used']}',
            '${p['lower_bound_bars_without_existing']}',
            ksh(p['full_bar_purchase_cost_ksh']),
            p['status'] == 'verified' ? 'verified' : 'UNCONFIRMED',
          ],
        [
          'Total',
          '${plan['total_new_bars']}',
          '',
          '',
          ksh(plan['full_bar_purchase_cost_ksh']),
          '',
        ],
      ],
      boldLast: true,
      notes: notes,
    );
  }

  static ReportBlock fittings(Json r) => ReportBlock(
    'Fittings and accessories',
    headers: const ['Item', 'Quantity', 'Unit price', 'Cost'],
    rows: [
      for (final l in (r['lines'] as List).cast<Json>())
        [
          fittingLabels[l['kind']] ?? '${l['kind']}',
          '${l['total_quantity']}',
          ksh(l['unit_price_ksh']),
          ksh(l['total_cost_ksh']),
        ],
      ['Total', '', '', ksh(r['total_cost_ksh'])],
    ],
    boldLast: true,
  );

  static ReportBlock labour(Json r) => ReportBlock(
    'Fundi labour',
    headers: const ['Windows', 'Count', 'Rate', 'Cost'],
    rows: [
      [
        '2 x 2 ft or smaller',
        '${r['small_windows']}',
        ksh(r['small_rate_ksh']),
        '',
      ],
      [
        'Larger than 2 x 2 ft',
        '${r['large_windows']}',
        ksh(r['large_rate_ksh']),
        '',
      ],
      ['Total labour', '', '', ksh(r['total_ksh'])],
    ],
    boldLast: true,
  );

  static ReportBlock transport(Json r) {
    final ok = r['status'] == 'confirmed';
    return ReportBlock(
      'Transport (once per order)',
      status: ok ? 'Confirmed' : 'UNCONFIRMED: enter the exact amount',
      headers: const ['Item', 'Amount'],
      rows: [
        [
          'Transport',
          ok
              ? ksh(r['amount_ksh'])
              : 'KSh ${fmt(r['min_ksh'], dp: 0)} - ${fmt(r['max_ksh'], dp: 0)}',
        ],
      ],
    );
  }

  static ReportBlock glassSheets(Json r) {
    final lines = (r['lines'] as List).cast<Json>();
    return ReportBlock(
      'Glass sheets (4 mm, 5.5 x 7 ft)',
      status: glassStatusText[r['status']] ?? '${r['status']}',
      headers: const ['Ordering size (ft)', 'Windows', 'Sheets', 'Status'],
      rows: [
        for (final l in lines)
          [
            '${l['rounded_width_ft']} x ${l['rounded_height_ft']}',
            '${l['windows']}',
            l['sheets'] == null ? 'ask fundi' : '${l['sheets']}',
            l['status'] == 'needs_fundi_confirmation'
                ? 'no confirmed yield'
                : '${l['status']}',
          ],
      ],
      notes: [
        if (r['total_sheets'] != null)
          'Total ${r['total_sheets']} sheet(s), ${fmt(r['billed_area_sqft'], dp: 0)} sq ft billed, ${ksh(r['cost_ksh'])}'
        else
          'No total: at least one size has no fundi-confirmed yield.',
        if (lines.any((l) => (l['bonus_2x2_windows_not_applied'] as num) > 0))
          'Spare 2 x 2 ft windows the yield allows are not applied to the order.',
      ],
    );
  }

  static ReportBlock total(String title, Json total) {
    final confirmed = total['confirmed'] == true;
    String range(String lo, String hi) => total[lo] == total[hi]
        ? fmt(total[lo])
        : '${fmt(total[lo])} - ${fmt(total[hi])}';
    return ReportBlock(
      title,
      status: confirmed ? 'Confirmed' : 'UNCONFIRMED',
      headers: const ['Cost line', 'KSh'],
      rows: [
        ['Aluminium', fmt(total['aluminium_ksh'])],
        ['Rubber (consumption)', fmt(total['rubber_ksh'])],
        ['Fittings and screws', fmt(total['fittings_ksh'])],
        ['Fundi labour', fmt(total['labour_ksh'])],
        if (total['glass_sheets_ksh'] != '0')
          ['Glass sheets', fmt(total['glass_sheets_ksh'])],
        ['Transport', range('transport_low_ksh', 'transport_high_ksh')],
        ['Estimated total', range('total_low_ksh', 'total_high_ksh')],
      ],
      boldLast: true,
      notes: (total['notes'] as List).cast<String>(),
    );
  }
}

/// All the blocks for a saved result, in the order the screen shows them.
List<ReportBlock> reportBlocks(CalcView view, Json result) {
  switch (view) {
    case CalcView.standard:
      return [
        Blocks.aluminium(result['aluminium'] as Json),
        Blocks.rubber(result['rubber'] as Json),
      ];
    case CalcView.aluminium:
      return [Blocks.aluminium(result)];
    case CalcView.rubber:
      return [Blocks.rubber(result)];
    case CalcView.rubberRolls:
      return [Blocks.rubberRolls(result)];
    case CalcView.stock:
      return [Blocks.stock('Aluminium bars to buy', result)];
    case CalcView.panels:
      return [Blocks.panels(result)];
    case CalcView.fittings:
      return [Blocks.fittings(result)];
    case CalcView.labour:
      return [Blocks.labour(result)];
    case CalcView.transport:
      return [Blocks.transport(result)];
    case CalcView.glassSheets:
      return [Blocks.glassSheets(result)];
    case CalcView.full:
      final b = result['stock_option_b'] as Json?;
      final u = result['existing_stock_utilisation'] as Json?;
      final g = result['glass_sheets'] as Json?;
      return [
        Blocks.aluminium(result['aluminium'] as Json),
        Blocks.rubber(result['rubber'] as Json),
        Blocks.panels(result['panels'] as Json),
        Blocks.stock(
          'Option A: buy all aluminium new',
          result['stock_option_a'] as Json,
        ),
        if (b != null)
          Blocks.stock('Option B: use existing stock, buy the rest', b),
        if (u != null)
          ReportBlock(
            'Existing stock used',
            notes: [
              '${u['pieces_used']} piece(s) supplying ${fmt(u['cut_length_mm'])} mm of cuts',
            ],
          ),
        Blocks.fittings(result['fittings'] as Json),
        Blocks.labour(result['labour'] as Json),
        Blocks.transport(result['transport'] as Json),
        if (g != null) Blocks.glassSheets(g),
        const ReportBlock(
          'Project totals',
          notes: [
            'Aluminium is costed one way per total, so nothing is counted twice. Rubber and fittings are actual consumption.',
          ],
        ),
        Blocks.total(
          'Aluminium actually used (consumption)',
          result['totals_consumption'] as Json,
        ),
        Blocks.total(
          'Aluminium as whole bars (Option A)',
          result['totals_whole_bars'] as Json,
        ),
        if (result['totals_existing_plus_purchase'] != null)
          Blocks.total(
            'Existing stock + bars still to buy (Option B)',
            result['totals_existing_plus_purchase'] as Json,
          ),
      ];
  }
}

String reportFileName(String? customer, int revision) {
  final base = (customer ?? 'order')
      .trim()
      .toLowerCase()
      .replaceAll(RegExp(r'[^a-z0-9]+'), '-')
      .replaceAll(RegExp(r'^-+|-+$'), '');
  return 'calculation-${base.isEmpty ? 'order' : base}-rev$revision.pdf';
}

/// Builds a PDF of the saved calculation: letterhead, order details and the
/// same tables the screen shows. [business] and [logo] are optional.
Future<Uint8List> buildReportPdf({
  required String? customer,
  required int revision,
  required String calculated,
  required int rulesVersion,
  required String viewLabel,
  required List<ReportBlock> blocks,
  Json? business,
  Uint8List? logo,
}) async {
  final regular = pw.Font.ttf(
    await rootBundle.load('assets/fonts/Inter-Regular.ttf'),
  );
  final bold = pw.Font.ttf(
    await rootBundle.load('assets/fonts/Inter-Bold.ttf'),
  );
  final theme = pw.ThemeData.withFont(base: regular, bold: bold);

  String b(String k) => '${business?[k] ?? ''}'.trim();
  final company = b('company_name').isEmpty
      ? 'AMBER STORES KENYA'
      : b('company_name');
  final contact = [
    b('phone'),
    b('email'),
    b('website'),
  ].where((x) => x.isNotEmpty).join('   |   ');
  final addressLines = b(
    'address',
  ).split('\n').map((l) => l.trim()).where((l) => l.isNotEmpty).toList();

  pw.MemoryImage? image;
  if (logo != null) {
    try {
      image = pw.MemoryImage(logo);
    } catch (_) {
      image = null; // a bad logo must not stop the report
    }
  }

  const navy = PdfColor.fromInt(0xFF1B3A5C);
  const headBg = PdfColor.fromInt(0xFFE7ECF3);
  const labelBg = PdfColor.fromInt(0xFFEEF1F6);
  const border = PdfColor.fromInt(0xFFD9DEE6);

  final doc = pw.Document(
    title: 'Calculation: ${customer ?? 'order'}',
    author: company,
  );
  doc.addPage(
    pw.MultiPage(
      theme: theme,
      pageFormat: PdfPageFormat.a4,
      margin: const pw.EdgeInsets.fromLTRB(40, 40, 40, 48),
      footer: (ctx) => pw.Row(
        mainAxisAlignment: pw.MainAxisAlignment.spaceBetween,
        children: [
          pw.Text(
            'Snapshot at rules version $rulesVersion. Later price changes do not alter these figures.',
            style: const pw.TextStyle(fontSize: 7, color: PdfColors.grey700),
          ),
          pw.Text(
            'Page ${ctx.pageNumber} of ${ctx.pagesCount}',
            style: const pw.TextStyle(fontSize: 8, color: PdfColors.grey700),
          ),
        ],
      ),
      build: (ctx) => [
        pw.Row(
          crossAxisAlignment: pw.CrossAxisAlignment.start,
          children: [
            if (image != null)
              pw.Container(
                width: 120,
                height: 56,
                margin: const pw.EdgeInsets.only(right: 12),
                child: pw.Image(
                  image,
                  fit: pw.BoxFit.contain,
                  alignment: pw.Alignment.topLeft,
                ),
              ),
            pw.Expanded(
              child: pw.Column(
                crossAxisAlignment: pw.CrossAxisAlignment.start,
                children: [
                  pw.Text(
                    company,
                    style: pw.TextStyle(font: bold, fontSize: 17, color: navy),
                  ),
                  for (final l in addressLines)
                    pw.Text(l, style: const pw.TextStyle(fontSize: 8.5)),
                  if (contact.isNotEmpty)
                    pw.Text(contact, style: const pw.TextStyle(fontSize: 8.5)),
                  if (b('tax_pin').isNotEmpty)
                    pw.Text(
                      'PIN: ${b('tax_pin')}',
                      style: const pw.TextStyle(fontSize: 8.5),
                    ),
                ],
              ),
            ),
          ],
        ),
        pw.Divider(color: navy, thickness: 1.2),
        pw.SizedBox(height: 6),
        pw.Text('CALCULATION', style: pw.TextStyle(font: bold, fontSize: 15)),
        pw.SizedBox(height: 6),
        pw.Table(
          border: pw.TableBorder.all(color: border, width: 0.6),
          columnWidths: {
            0: const pw.FlexColumnWidth(1.1),
            1: const pw.FlexColumnWidth(2),
            2: const pw.FlexColumnWidth(1.1),
            3: const pw.FlexColumnWidth(2),
          },
          children: [
            for (final pair in [
              (
                ['Customer', customer ?? 'No customer name'],
                ['Revision', '$revision'],
              ),
              (['Calculated', calculated], ['Rules version', 'v$rulesVersion']),
              (['Report view', viewLabel], ['', '']),
            ])
              pw.TableRow(
                children: [
                  for (final cell in [...pair.$1, ...pair.$2].indexed)
                    pw.Container(
                      color: cell.$1.isEven ? labelBg : null,
                      padding: const pw.EdgeInsets.symmetric(
                        horizontal: 6,
                        vertical: 4,
                      ),
                      child: pw.Text(
                        cell.$2,
                        style: pw.TextStyle(
                          fontSize: 9,
                          font: cell.$1.isEven ? bold : regular,
                        ),
                      ),
                    ),
                ],
              ),
          ],
        ),
        pw.SizedBox(height: 14),
        for (final blk in blocks) ...[
          pw.Text(
            blk.title,
            style: pw.TextStyle(font: bold, fontSize: 11.5, color: navy),
          ),
          if (blk.status != null)
            pw.Padding(
              padding: const pw.EdgeInsets.only(top: 2),
              child: pw.Text(
                blk.status!,
                style: pw.TextStyle(
                  font: bold,
                  fontSize: 8.5,
                  color: blk.status!.startsWith('UNCONFIRMED')
                      ? PdfColors.orange800
                      : PdfColors.green800,
                ),
              ),
            ),
          pw.SizedBox(height: 4),
          if (blk.rows.isNotEmpty)
            pw.TableHelper.fromTextArray(
              headers: blk.headers,
              data: blk.rows,
              border: pw.TableBorder.all(color: border, width: 0.6),
              headerDecoration: const pw.BoxDecoration(color: headBg),
              headerStyle: pw.TextStyle(font: bold, fontSize: 9),
              cellStyle: const pw.TextStyle(fontSize: 9),
              cellPadding: const pw.EdgeInsets.symmetric(
                horizontal: 6,
                vertical: 4,
              ),
              cellAlignments: {
                for (var i = 1; i < blk.headers.length; i++)
                  i: pw.Alignment.centerRight,
              },
              headerAlignments: {
                for (var i = 1; i < blk.headers.length; i++)
                  i: pw.Alignment.centerRight,
              },
              oddRowDecoration: const pw.BoxDecoration(
                color: PdfColor.fromInt(0xFFF8F9FB),
              ),
              rowDecoration: null,
              cellDecoration: blk.boldLast
                  ? (index, data, rowNum) => rowNum == blk.rows.length - 1
                        ? const pw.BoxDecoration(color: labelBg)
                        : const pw.BoxDecoration()
                  : null,
              cellFormat: (index, data) => '$data',
            ),
          for (final n in blk.notes)
            pw.Padding(
              padding: const pw.EdgeInsets.only(top: 3),
              child: pw.Text(n, style: const pw.TextStyle(fontSize: 8.5)),
            ),
          pw.SizedBox(height: 14),
        ],
      ],
    ),
  );
  return doc.save();
}

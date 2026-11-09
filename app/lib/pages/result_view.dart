import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';
import '../format.dart';
import '../report.dart';
import '../theme.dart';
import '../widgets.dart';

/// Renders an engine result. Every figure shown is a string the engine produced.
class ResultView extends StatelessWidget {
  const ResultView({super.key, required this.view, required this.result});

  final CalcView view;
  final Json result;

  @override
  Widget build(BuildContext context) {
    final children = switch (view) {
      CalcView.standard => [
        AluminiumTable(result['aluminium'] as Json),
        RubberCard(result['rubber'] as Json),
      ],
      CalcView.aluminium => [AluminiumTable(result)],
      CalcView.rubber => [RubberCard(result)],
      CalcView.rubberRolls => [BlockCard(Blocks.rubberRolls(result))],
      CalcView.stock => [
        StockSection(title: 'Aluminium bars to buy', plan: result),
      ],
      CalcView.panels => [PanelsSection(result)],
      CalcView.fittings => [FittingsSection(result)],
      CalcView.labour => [LabourSection(result)],
      CalcView.transport => [TransportSection(result)],
      CalcView.glassSheets => [GlassSheetsSection(result)],
      CalcView.full => _full(result),
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: children,
    );
  }

  List<Widget> _full(Json r) {
    final b = r['stock_option_b'] as Json?;
    final u = r['existing_stock_utilisation'] as Json?;
    final g = r['glass_sheets'] as Json?;
    return [
      AluminiumTable(r['aluminium'] as Json),
      RubberCard(r['rubber'] as Json),
      PanelsSection(r['panels'] as Json),
      StockSection(
        title: 'Option A: buy all aluminium new',
        plan: r['stock_option_a'] as Json,
      ),
      if (b != null)
        StockSection(
          title: 'Option B: use existing stock, buy the rest',
          plan: b,
        ),
      if (u != null)
        Section(
          title: 'Existing stock used',
          child: Text(
            '${u['pieces_used']} piece(s) supplying ${fmt(u['cut_length_mm'])} mm of cuts',
          ),
        ),
      FittingsSection(r['fittings'] as Json),
      LabourSection(r['labour'] as Json),
      TransportSection(r['transport'] as Json),
      if (g != null) GlassSheetsSection(g),
      Section(
        title: 'Project totals',
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Aluminium is costed one way per total, so nothing is counted twice. '
              'Rubber and fittings are actual consumption.',
              style: const TextStyle(fontStyle: FontStyle.italic),
            ),
            const SizedBox(height: 8),
            TotalCard(
              title: 'Aluminium actually used (consumption)',
              total: r['totals_consumption'] as Json,
            ),
            TotalCard(
              title: 'Aluminium as whole bars (Option A)',
              total: r['totals_whole_bars'] as Json,
            ),
            if (r['totals_existing_plus_purchase'] != null)
              TotalCard(
                title: 'Existing stock + bars still to buy (Option B)',
                total: r['totals_existing_plus_purchase'] as Json,
              ),
          ],
        ),
      ),
    ];
  }
}

class Section extends StatelessWidget {
  const Section({super.key, required this.title, required this.child});
  final String title;
  final Widget child;

  @override
  Widget build(BuildContext context) => Padding(
    padding: const EdgeInsets.only(bottom: 14),
    child: AppCard(title: title, child: child),
  );
}

class StatusChip extends StatelessWidget {
  const StatusChip(this.text, {super.key, required this.ok});
  final String text;
  final bool ok;

  @override
  Widget build(BuildContext context) {
    final color = ok ? Brand.ok : Brand.warn;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: color),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 7,
            height: 7,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 5),
          Text(
            text,
            style: TextStyle(
              color: color,
              fontSize: 12,
              fontWeight: FontWeight.w700,
            ),
          ),
        ],
      ),
    );
  }
}

/// Simple scrollable table. Numeric columns are right-aligned.
class Tbl extends StatelessWidget {
  const Tbl({
    super.key,
    required this.headers,
    required this.rows,
    this.boldLast = false,
  });
  final List<String> headers;
  final List<List<String>> rows;
  final bool boldLast;

  @override
  Widget build(BuildContext context) {
    TableRow row(
      List<String> cells, {
      bool header = false,
      bool bold = false,
      bool zebra = false,
    }) => TableRow(
      decoration: BoxDecoration(
        color: header
            ? Brand.headBg
            : (bold ? Brand.labelBg : (zebra ? const Color(0xFFF8F9FB) : null)),
      ),
      children: [
        for (var i = 0; i < cells.length; i++)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 7),
            child: Text(
              cells[i],
              textAlign: i == 0 ? TextAlign.left : TextAlign.right,
              style: TextStyle(
                fontWeight: header || bold
                    ? FontWeight.w700
                    : FontWeight.normal,
              ),
            ),
          ),
      ],
    );
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Table(
        defaultColumnWidth: const IntrinsicColumnWidth(),
        border: TableBorder.all(color: Brand.border),
        children: [
          row(headers, header: true),
          for (var i = 0; i < rows.length; i++)
            row(
              rows[i],
              bold: boldLast && i == rows.length - 1,
              zebra: i.isOdd,
            ),
        ],
      ),
    );
  }
}

/// A titled table (and notes) taken straight from a [ReportBlock].
class BlockCard extends StatelessWidget {
  const BlockCard(this.block, {super.key});
  final ReportBlock block;

  @override
  Widget build(BuildContext context) => Section(
    title: block.title,
    child: Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Tbl(headers: block.headers, rows: block.rows, boldLast: block.boldLast),
        for (final n in block.notes)
          Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(n, style: Theme.of(context).textTheme.bodySmall),
          ),
      ],
    ),
  );
}

class AluminiumTable extends StatelessWidget {
  const AluminiumTable(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final b = Blocks.aluminium(r);
    return Section(
      title: b.title,
      child: Tbl(headers: b.headers, rows: b.rows, boldLast: b.boldLast),
    );
  }
}

class RubberCard extends StatelessWidget {
  const RubberCard(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final b = Blocks.rubber(r);
    return Section(
      title: b.title,
      child: Tbl(headers: b.headers, rows: b.rows, boldLast: b.boldLast),
    );
  }
}

class PanelsSection extends StatelessWidget {
  const PanelsSection(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final b = Blocks.panels(r);
    return Section(
      title: b.title,
      child: Tbl(headers: b.headers, rows: b.rows, boldLast: b.boldLast),
    );
  }
}

class StockSection extends StatelessWidget {
  const StockSection({super.key, required this.title, required this.plan});
  final String title;
  final Json plan;

  @override
  Widget build(BuildContext context) {
    final verified = plan['status'] == 'verified';
    final profiles = (plan['profiles'] as List).cast<Json>();
    return Section(
      title: title,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Wrap(
            spacing: 8,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              StatusChip(
                verified
                    ? 'Cutting plan verified'
                    : 'UNCONFIRMED: cutting plan could not be verified',
                ok: verified,
              ),
              if (plan['kerf_mm'] != '0')
                Text('Saw kerf ${plan['kerf_mm']} mm per cut'),
            ],
          ),
          const SizedBox(height: 8),
          Tbl(
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
          ),
          const SizedBox(height: 4),
          Text(
            '*Total length divided by bar length. This is only a floor: the bars to buy come from the physical cutting plan.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
          for (final p in profiles) ...[
            for (final i in (p['issues'] as List).cast<Json>())
              Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  '${profileLabels[p['profile']]}: ${issueText(i)}',
                  style: TextStyle(color: Colors.orange.shade900),
                ),
              ),
          ],
          const SizedBox(height: 4),
          for (final p in profiles) _CutPlanTile(p),
        ],
      ),
    );
  }
}

class _CutPlanTile extends StatelessWidget {
  const _CutPlanTile(this.p);
  final Json p;

  @override
  Widget build(BuildContext context) {
    final bins = (p['bins'] as List).cast<Json>();
    final shortage = p['shortage_length_mm'];
    return ExpansionTile(
      tilePadding: EdgeInsets.zero,
      title: Text(
        '${profileLabels[p['profile']]}: cutting plan (${bins.length} bar/piece${bins.length == 1 ? '' : 's'})',
      ),
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              for (var i = 0; i < bins.length; i++)
                Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Text(
                    '${bins[i]['source'] == 'new' ? 'New bar' : 'Existing ${fmt(bins[i]['capacity_mm'])} mm'} ${i + 1}: '
                    '${(bins[i]['cuts_mm'] as List).map((c) => fmt(c, group: false)).join(' + ')} mm '
                    '(left over ${fmt(bins[i]['remaining_mm'])} mm)',
                  ),
                ),
              if (p['existing_available_mm'] != '0')
                Text(
                  'Confirmed existing stock ${fmt(p['existing_available_mm'])} mm; length-based shortage ${fmt(shortage)} mm '
                  '(${fmt(p['shortage_length_ft'])} ft, about ${ksh(p['shortage_cost_proportional_ksh'])} at the per-foot rate; indicative only).',
                ),
              if ((p['ignored_unconfirmed_pieces'] as num) > 0)
                Text(
                  '${p['ignored_unconfirmed_pieces']} unconfirmed existing piece(s) were ignored.',
                  style: TextStyle(color: Colors.orange.shade900),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

class FittingsSection extends StatelessWidget {
  const FittingsSection(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final b = Blocks.fittings(r);
    return Section(
      title: b.title,
      child: Tbl(headers: b.headers, rows: b.rows, boldLast: b.boldLast),
    );
  }
}

class LabourSection extends StatelessWidget {
  const LabourSection(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final b = Blocks.labour(r);
    return Section(
      title: b.title,
      child: Tbl(headers: b.headers, rows: b.rows, boldLast: b.boldLast),
    );
  }
}

class TransportSection extends StatelessWidget {
  const TransportSection(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final ok = r['status'] == 'confirmed';
    return Section(
      title: 'Transport (once per order)',
      child: Wrap(
        spacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          Text(
            ok
                ? ksh(r['amount_ksh'])
                : 'KSh ${fmt(r['min_ksh'], dp: 0)} - ${fmt(r['max_ksh'], dp: 0)}',
            style: const TextStyle(fontWeight: FontWeight.w700),
          ),
          StatusChip(
            ok ? 'Confirmed' : 'UNCONFIRMED: enter the exact amount',
            ok: ok,
          ),
        ],
      ),
    );
  }
}

class GlassSheetsSection extends StatelessWidget {
  const GlassSheetsSection(this.r, {super.key});
  final Json r;

  @override
  Widget build(BuildContext context) {
    final ok = r['status'] == 'confirmed';
    final lines = (r['lines'] as List).cast<Json>();
    return Section(
      title: 'Glass sheets (4 mm, 5.5 x 7 ft)',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          StatusChip(glassStatusText[r['status']] ?? '${r['status']}', ok: ok),
          const SizedBox(height: 8),
          Tbl(
            headers: const [
              'Ordering size (ft)',
              'Windows',
              'Sheets',
              'Status',
            ],
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
          ),
          const SizedBox(height: 4),
          if (r['total_sheets'] != null)
            Text(
              'Total ${r['total_sheets']} sheet(s), ${fmt(r['billed_area_sqft'], dp: 0)} sq ft billed, ${ksh(r['cost_ksh'])}',
            )
          else
            const Text(
              'No total: at least one size has no fundi-confirmed yield.',
            ),
          if (lines.any((l) => (l['bonus_2x2_windows_not_applied'] as num) > 0))
            Text(
              'Spare 2 x 2 ft windows the yield allows are not applied to the order.',
              style: Theme.of(context).textTheme.bodySmall,
            ),
        ],
      ),
    );
  }
}

class TotalCard extends StatelessWidget {
  const TotalCard({super.key, required this.title, required this.total});
  final String title;
  final Json total;

  @override
  Widget build(BuildContext context) {
    final confirmed = total['confirmed'] == true;
    String range(String lo, String hi) => total[lo] == total[hi]
        ? fmt(total[lo])
        : '${fmt(total[lo])} - ${fmt(total[hi])}';
    final notes = (total['notes'] as List).cast<String>();
    return Card(
      margin: const EdgeInsets.only(bottom: 10),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Text(
                    title,
                    style: const TextStyle(fontWeight: FontWeight.w700),
                  ),
                ),
                StatusChip(
                  confirmed ? 'Confirmed' : 'UNCONFIRMED',
                  ok: confirmed,
                ),
              ],
            ),
            const SizedBox(height: 8),
            Tbl(
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
            ),
            for (final n in notes)
              Padding(
                padding: const EdgeInsets.only(top: 6),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Icon(
                      Icons.warning_amber_rounded,
                      size: 16,
                      color: Brand.warn,
                    ),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        n,
                        style: TextStyle(color: Colors.orange.shade900),
                      ),
                    ),
                  ],
                ),
              ),
          ],
        ),
      ),
    );
  }
}

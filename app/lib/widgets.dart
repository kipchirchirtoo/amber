import 'package:flutter/material.dart';

import 'theme.dart';

/// White card with an icon + title header and a collapse button, like the
/// panels on an ERP order screen. Starts open; the header toggles it.
class AppCard extends StatefulWidget {
  const AppCard({
    super.key,
    required this.title,
    required this.child,
    this.icon,
    this.trailing,
    this.collapsible = true,
    this.padding = const EdgeInsets.all(12),
  });
  final String title;
  final Widget child;
  final IconData? icon;
  final Widget? trailing;
  final bool collapsible;
  final EdgeInsets padding;

  @override
  State<AppCard> createState() => _AppCardState();
}

class _AppCardState extends State<AppCard> {
  bool _open = true;

  @override
  Widget build(BuildContext context) {
    return Card(
      clipBehavior: Clip.antiAlias,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          InkWell(
            onTap: widget.collapsible
                ? () => setState(() {
                    _open = !_open;
                  })
                : null,
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
              decoration: const BoxDecoration(
                color: Color(0xFFF8F9FB),
                border: Border(bottom: BorderSide(color: Brand.border)),
              ),
              child: Row(
                children: [
                  if (widget.collapsible)
                    Tooltip(
                      message: _open ? 'Collapse' : 'Expand',
                      child: Icon(
                        _open ? Icons.expand_more : Icons.chevron_right,
                        size: 20,
                        color: Brand.navy,
                      ),
                    ),
                  if (widget.icon != null) ...[
                    const SizedBox(width: 4),
                    Icon(widget.icon, size: 18, color: Brand.navy),
                  ],
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      widget.title,
                      style: const TextStyle(
                        fontWeight: FontWeight.w700,
                        fontSize: 14,
                        color: Brand.navy,
                      ),
                    ),
                  ),
                  if (widget.trailing != null) widget.trailing!,
                ],
              ),
            ),
          ),
          if (_open) Padding(padding: widget.padding, child: widget.child),
        ],
      ),
    );
  }
}

/// Tinted-square icon, big number and caption. [tone] colours the icon and
/// number: use green/amber/red only to say ok / check / problem.
class KpiCard extends StatelessWidget {
  const KpiCard({
    super.key,
    required this.icon,
    required this.value,
    required this.label,
    this.sub,
    this.tone = Brand.blue,
    this.onTap,
  });
  final IconData icon;
  final String value;
  final String label;
  final String? sub;
  final Color tone;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(14),
          child: Row(
            children: [
              Container(
                width: 44,
                height: 44,
                decoration: BoxDecoration(
                  color: tone.withValues(alpha: 0.12),
                  borderRadius: BorderRadius.circular(8),
                ),
                child: Icon(icon, color: tone),
              ),
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      label,
                      style: const TextStyle(fontSize: 12, color: Brand.muted),
                    ),
                    Text(
                      value,
                      style: const TextStyle(
                        fontSize: 24,
                        fontWeight: FontWeight.w700,
                        height: 1.15,
                      ),
                    ),
                    if (sub != null)
                      Text(
                        sub!,
                        style: TextStyle(
                          fontSize: 12,
                          color: tone,
                          fontWeight: FontWeight.w600,
                        ),
                      ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Label / value pairs in a bordered grid with a shaded label column (two
/// pairs per row when there is room), as in the "Order Information" panel.
class KvGrid extends StatelessWidget {
  const KvGrid(this.pairs, {super.key});
  final List<(String, Widget)> pairs;

  static Widget text(String s, {bool bold = false}) => Text(
    s,
    style: TextStyle(fontWeight: bold ? FontWeight.w700 : FontWeight.normal),
  );

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, c) {
        final two = c.maxWidth >= 640;
        final rows = <TableRow>[];
        Widget label(String s) => Container(
          color: Brand.labelBg,
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: Text(s, style: const TextStyle(fontWeight: FontWeight.w700)),
        );
        Widget value(Widget w) => Padding(
          padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
          child: w,
        );
        if (two) {
          for (var i = 0; i < pairs.length; i += 2) {
            final a = pairs[i];
            final b = i + 1 < pairs.length ? pairs[i + 1] : null;
            rows.add(
              TableRow(
                children: [
                  label(a.$1),
                  value(a.$2),
                  if (b != null) label(b.$1) else const SizedBox.shrink(),
                  if (b != null) value(b.$2) else const SizedBox.shrink(),
                ],
              ),
            );
          }
        } else {
          for (final p in pairs) {
            rows.add(TableRow(children: [label(p.$1), value(p.$2)]));
          }
        }
        return Table(
          columnWidths: two
              ? const {
                  0: FlexColumnWidth(1.2),
                  1: FlexColumnWidth(2),
                  2: FlexColumnWidth(1.2),
                  3: FlexColumnWidth(2),
                }
              : const {0: FlexColumnWidth(1.2), 1: FlexColumnWidth(2)},
          border: TableBorder.all(color: Brand.border),
          children: rows,
        );
      },
    );
  }
}

/// Page title with a row of small actions on the right.
class PageHeader extends StatelessWidget {
  const PageHeader(
    this.title, {
    super.key,
    this.actions = const [],
    this.subtitle,
  });
  final String title;
  final String? subtitle;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Wrap(
        spacing: 12,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        alignment: WrapAlignment.spaceBetween,
        children: [
          Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(
                title,
                style: const TextStyle(
                  fontSize: 24,
                  fontWeight: FontWeight.w700,
                ),
              ),
              if (subtitle != null)
                Text(subtitle!, style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
          Wrap(spacing: 8, runSpacing: 8, children: actions),
        ],
      ),
    );
  }
}

/// Small outlined action button, like the "Change Status / Costs / Duplicate" strip.
class MiniButton extends StatelessWidget {
  const MiniButton(this.label, {super.key, this.icon, required this.onPressed});
  final String label;
  final IconData? icon;
  final VoidCallback? onPressed;

  @override
  Widget build(BuildContext context) => OutlinedButton.icon(
    onPressed: onPressed,
    icon: icon == null ? const SizedBox.shrink() : Icon(icon, size: 16),
    label: Text(label),
    style: OutlinedButton.styleFrom(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
      minimumSize: const Size(0, 34),
    ),
  );
}

/// Coloured dot + text for a status column.
class StatusDot extends StatelessWidget {
  const StatusDot(this.text, this.color, {super.key});
  final String text;
  final Color color;

  @override
  Widget build(BuildContext context) => Row(
    mainAxisSize: MainAxisSize.min,
    children: [
      Container(
        width: 9,
        height: 9,
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      ),
      const SizedBox(width: 6),
      Text(text),
    ],
  );
}

/// Round gauge with the value in the middle (like the OEE rings on a plant
/// dashboard). Colour is chosen by the caller: green ok, amber check, red problem.
class RingGauge extends StatelessWidget {
  const RingGauge({
    super.key,
    required this.percent,
    required this.label,
    this.size = 84,
  });

  /// 0 to 100, or null when there is nothing to measure yet.
  final int? percent;
  final String label;
  final double size;

  static Color toneFor(int p) =>
      p >= 80 ? Brand.ok : (p >= 50 ? Brand.warn : Brand.bad);

  @override
  Widget build(BuildContext context) {
    final p = percent;
    final tone = p == null ? Brand.border : toneFor(p);
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        SizedBox(
          width: size,
          height: size,
          child: Stack(
            alignment: Alignment.center,
            children: [
              SizedBox.expand(
                child: CircularProgressIndicator(
                  value: (p ?? 0) / 100,
                  strokeWidth: 9,
                  backgroundColor: const Color(0xFFE3E7EE),
                  color: tone,
                ),
              ),
              Text(
                p == null ? 'n/a' : '$p',
                style: TextStyle(
                  fontSize: size * 0.28,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 6),
        Text(label, style: const TextStyle(fontSize: 12, color: Brand.muted)),
      ],
    );
  }
}

/// Initials in a circle, used for the signed-in person.
class UserAvatar extends StatelessWidget {
  const UserAvatar(this.name, {super.key, this.radius = 17});
  final String name;
  final double radius;

  static String initials(String name) {
    final parts = name
        .trim()
        .split(RegExp(r'[\s@._-]+'))
        .where((p) => p.isNotEmpty)
        .toList();
    if (parts.isEmpty) return '?';
    if (parts.length == 1) {
      return parts.first
          .substring(0, parts.first.length >= 2 ? 2 : 1)
          .toUpperCase();
    }
    return (parts.first[0] + parts[1][0]).toUpperCase();
  }

  @override
  Widget build(BuildContext context) => CircleAvatar(
    radius: radius,
    backgroundColor: const Color(0xFFDCE8FB),
    child: Text(
      initials(name),
      style: TextStyle(
        color: Brand.navy,
        fontWeight: FontWeight.w700,
        fontSize: radius * 0.75,
      ),
    ),
  );
}

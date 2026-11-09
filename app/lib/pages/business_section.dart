import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';

/// Letterhead, payment details and logo that print on every quotation.
/// Everyone can see them; only administrators can change them.
class BusinessSection extends StatefulWidget {
  const BusinessSection({super.key, required this.controller});
  final AppController controller;

  @override
  State<BusinessSection> createState() => _BusinessSectionState();
}

class _BusinessSectionState extends State<BusinessSection> {
  static const _short = {
    'company_name': 'Company name',
    'phone': 'Phone',
    'email': 'Email',
    'website': 'Website',
    'tax_pin': 'KRA PIN (optional)',
  };
  static const _long = {
    'address': 'Address (one line per row)',
    'payment_details': 'How to pay (M-Pesa Paybill / Till, bank account...)',
    'default_payment_terms':
        'Default payment terms (used unless you type others)',
    'footer_note': 'Footer note (optional)',
  };

  final _c = <String, TextEditingController>{};
  final _days = TextEditingController();
  Uint8List? _logo;
  bool _loading = true;
  String? _error;
  String? _ok;

  ApiClient get api => widget.controller.api;
  bool get _admin => widget.controller.isAdmin;

  @override
  void initState() {
    super.initState();
    for (final k in [..._short.keys, ..._long.keys]) {
      _c[k] = TextEditingController();
    }
    _load();
  }

  @override
  void dispose() {
    for (final c in _c.values) {
      c.dispose();
    }
    _days.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final p = await api.getBusiness();
      final logo = await api.getLogo();
      if (!mounted) return;
      setState(() {
        for (final e in _c.entries) {
          e.value.text = '${p[e.key] ?? ''}';
        }
        _days.text = p['quote_valid_days'] == null
            ? ''
            : '${p['quote_valid_days']}';
        _logo = logo;
        _loading = false;
      });
    } on ApiException catch (e) {
      if (mounted) {
        setState(() {
          _error = e.message;
          _loading = false;
        });
      }
    }
  }

  Future<void> _save() async {
    setState(() {
      _error = null;
      _ok = null;
    });
    final days = _days.text.trim();
    if (days.isNotEmpty && int.tryParse(days) == null) {
      return setState(
        () => _error = 'Quote validity must be a whole number of days.',
      );
    }
    try {
      await api.saveBusiness({
        for (final e in _c.entries) e.key: e.value.text.trim(),
        'quote_valid_days': days.isEmpty ? null : int.parse(days),
      });
      setState(() => _ok = 'Saved. New quotations use these details.');
    } on ApiException catch (e) {
      setState(() => _error = e.message);
    }
  }

  Future<void> _pickLogo() async {
    setState(() {
      _error = null;
      _ok = null;
    });
    try {
      final f = await FilePicker.pickFile(
        type: FileType.custom,
        allowedExtensions: const ['jpg', 'jpeg'],
        dialogTitle: 'Choose the logo (JPEG)',
      );
      if (f == null) return;
      final bytes = await f.readAsBytes();
      await api.uploadLogo(bytes);
      setState(() {
        _logo = bytes;
        _ok = 'Logo saved.';
      });
    } on ApiException catch (e) {
      setState(() => _error = e.message);
    }
  }

  Future<void> _removeLogo() async {
    try {
      await api.deleteLogo();
      setState(() {
        _logo = null;
        _ok = 'Logo removed.';
      });
    } on ApiException catch (e) {
      setState(() => _error = e.message);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_loading) return const LinearProgressIndicator();
    Widget field(
      String key,
      String label, {
      double width = 260,
      int lines = 1,
    }) => SizedBox(
      width: width,
      child: TextField(
        controller: _c[key],
        readOnly: !_admin,
        minLines: lines,
        maxLines: lines == 1 ? 1 : lines + 2,
        decoration: InputDecoration(
          labelText: label,
          isDense: true,
          border: const OutlineInputBorder(),
        ),
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            Container(
              width: 150,
              height: 60,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                border: Border.all(color: Theme.of(context).dividerColor),
              ),
              child: _logo == null
                  ? const Text('No logo')
                  : Image.memory(_logo!, fit: BoxFit.contain),
            ),
            const SizedBox(width: 12),
            if (_admin) ...[
              OutlinedButton.icon(
                onPressed: _pickLogo,
                icon: const Icon(Icons.image_outlined),
                label: const Text('Choose logo (JPEG)'),
              ),
              const SizedBox(width: 8),
              if (_logo != null)
                TextButton(onPressed: _removeLogo, child: const Text('Remove')),
            ],
          ],
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            for (final e in _short.entries) field(e.key, e.value),
            SizedBox(
              width: 200,
              child: TextField(
                controller: _days,
                readOnly: !_admin,
                keyboardType: TextInputType.number,
                decoration: const InputDecoration(
                  labelText: 'Quote valid for (days)',
                  isDense: true,
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            for (final e in _long.entries)
              field(e.key, e.value, width: 540, lines: 3),
          ],
        ),
        const SizedBox(height: 8),
        if (_admin)
          FilledButton(
            onPressed: _save,
            child: const Text('Save letterhead details'),
          ),
        Padding(
          padding: const EdgeInsets.only(top: 6),
          child: Text(
            'Plain letters, digits and basic punctuation print best on the PDF. Anything else may print as "?". Changing these changes how earlier quotations print when they are opened again.',
            style: Theme.of(context).textTheme.bodySmall,
          ),
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(
              _error!,
              style: TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ),
        if (_ok != null)
          Padding(padding: const EdgeInsets.only(top: 6), child: Text(_ok!)),
      ],
    );
  }
}

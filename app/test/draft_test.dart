import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('builds the request the server expects', () {
    final d = OrderDraft();
    d.windows.first
      ..size.text = '4 x 4 ft'
      ..qty.text = '5';
    d.addWindow();
    d.windows.last
      ..size.text = '2 x 2 ft'
      ..qty.text = '2'
      ..label.text = 'toilet';
    d.transport.text = '800';
    d.addStock();
    d.stock.first
      ..length.text = '8 ft'
      ..confirmed = true;

    final r = d.toRequest(CalcView.full);
    expect(r['view'], 'full');
    expect(r['windows'], [
      {'size': '4 x 4 ft', 'quantity': 5},
      {'size': '2 x 2 ft', 'quantity': 2, 'label': 'toilet'},
    ]);
    expect(r['options'], {
      'transport_exact_ksh': '800',
      'existing_stock': [
        {
          'profile': 'side_track',
          'length': '8 ft',
          'quantity': 1,
          'confirmed': true,
        },
      ],
    });
    expect(r.containsKey('default_unit'), false);
  });

  test(
    'unconfirmed stock is sent as unconfirmed, never defaulted to confirmed',
    () {
      final d = OrderDraft();
      d.windows.first.size.text = '4 x 4 ft';
      d.addStock();
      d.stock.first.length.text = '8 ft';
      final r = d.toRequest(CalcView.stock);
      expect((r['options'] as Map)['existing_stock'][0]['confirmed'], false);
    },
  );

  test('refuses an empty order or a bad quantity with a plain message', () {
    final d = OrderDraft();
    expect(() => d.toRequest(CalcView.standard), throwsA(isA<ApiException>()));
    d.windows.first
      ..size.text = '4 x 4 ft'
      ..qty.text = '0';
    expect(() => d.toRequest(CalcView.standard), throwsA(isA<ApiException>()));
    d.windows.first.qty.text = '2.5';
    expect(() => d.toRequest(CalcView.standard), throwsA(isA<ApiException>()));
  });

  test('a saved request loads back into the form', () {
    final d = OrderDraft();
    d.windows.first
      ..size.text = '1200 x 1500 mm'
      ..qty.text = '3';
    d.defaultUnit = 'mm';
    d.kerf.text = '3';
    final req = d.toRequest(CalcView.stock);
    final e = OrderDraft()..loadRequest(req, customerName: 'Mama Wanjiru');
    expect(e.toRequest(CalcView.stock), req);
    expect(e.customerName, 'Mama Wanjiru');
  });
}

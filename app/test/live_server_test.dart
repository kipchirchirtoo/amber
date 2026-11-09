// Runs the real ApiClient against a running amber_server.
//   AMBER_LIVE_URL=http://localhost:8080 flutter test test/live_server_test.dart
import 'dart:io';
import 'dart:typed_data';

import 'package:amber_app/api.dart';
import 'package:amber_app/draft.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:http/http.dart' as http;

void main() {
  final url = Platform.environment['AMBER_LIVE_URL'];
  final user = Platform.environment['AMBER_LIVE_USER'] ?? 'boss';
  final password = Platform.environment['AMBER_LIVE_PASSWORD'] ?? '';

  test(
    'draft -> server -> engine result, then save and reload',
    () async {
      final api = ApiClient(baseUrl: url!);
      expect(await api.health(), true);

      // nothing works until someone signs in
      await expectLater(
        ApiClient(baseUrl: url).listOrders(),
        throwsA(
          isA<ApiException>().having(
            (e) => e.unauthorized,
            'unauthorized',
            true,
          ),
        ),
      );
      await expectLater(
        api.login(user, 'definitely wrong'),
        throwsA(
          isA<ApiException>().having(
            (e) => e.message,
            'message',
            contains('wrong username or password'),
          ),
        ),
      );
      final me = await api.login(user, password);
      expect(me['username'], user);

      final d = OrderDraft();
      d.windows.first
        ..size.text = '4 x 4 ft'
        ..qty.text = '5';
      d.addWindow();
      d.windows.last
        ..size.text = '5 x 4 ft'
        ..qty.text = '3';
      d.addWindow();
      d.windows.last
        ..size.text = '2 x 2 ft'
        ..qty.text = '2';

      final std = await api.calculate(d.toRequest(CalcView.standard));
      expect(std['result']['aluminium']['grand_total_length_mm'], '112585.6');
      expect(std['result']['rubber']['total_metres'], '60.2656');

      d.transport.text = '800';
      final full = await api.calculate(d.toRequest(CalcView.full));
      expect(full['result']['labour']['total_ksh'], '6600');
      expect(full['result']['totals_consumption']['confirmed'], true);

      final saved = await api.createOrder(
        'Live test',
        d.toRequest(CalcView.full),
      );
      final again = await api.getOrder(saved['id'] as String);
      expect(again['latest']['result'], saved['latest']['result']);
      expect((await api.listOrders()).any((o) => o['id'] == saved['id']), true);

      // bad input comes back as a readable message
      d.windows.first.size.text = '4 x 4';
      await expectLater(
        api.calculate(d.toRequest(CalcView.standard)),
        throwsA(
          isA<ApiException>().having(
            (e) => e.message,
            'message',
            contains('no unit'),
          ),
        ),
      );

      final rules = await api.currentRules();
      expect(rules['version_no'], greaterThanOrEqualTo(1));

      // assistant: off unless the server has ANTHROPIC_API_KEY; either way it must answer clearly
      if (!await api.assistantEnabled()) {
        await expectLater(
          api.assistantChat(null, 'hello'),
          throwsA(
            isA<ApiException>().having(
              (e) => e.message,
              'message',
              contains('ANTHROPIC_API_KEY'),
            ),
          ),
        );
      }

      // quotation with the owner's margin and terms
      final q = await api.getQuote(
        saved['id'] as String,
        extra: {'margin_percent': '20', 'deposit_percent': '50'},
      );
      expect(q['selling']['selling_price_ksh'], '60044.14');

      // stock list: add, confirm, remove
      final it = await api.saveStock(null, {
        'profile': 'side_track',
        'length': '8 ft',
        'quantity': 1,
        'confirmed': false,
      });
      expect(it['length_mm'], '2438.4');
      await api.saveStock(it['id'] as String, {
        'profile': 'side_track',
        'length_mm': it['length_mm'],
        'quantity': 1,
        'confirmed': true,
      });
      expect((await api.listStock()).single['confirmed'], true);
      await api.deleteStock(it['id'] as String);
      expect(await api.listStock(), isEmpty);

      // administrator: prices, letterhead, logo, and a quotation PDF opened with a ticket
      final r = Map<String, dynamic>.from(rules['rules'] as Map)
        ..['stock_price_ksh'] = '2100';
      final v = await api.updateRules(r, 'live test price change');
      expect(v['version_no'], rules['version_no'] + 1);
      expect(v['changed_by'], user);

      await api.saveBusiness({
        'company_name': 'Amber Stores Kenya',
        'payment_details': 'M-Pesa Paybill 123456',
        'quote_valid_days': 14,
      });
      expect((await api.getBusiness())['company_name'], 'Amber Stores Kenya');
      final jpeg = Uint8List.fromList([
        0xFF,
        0xD8,
        0xFF,
        0xC0,
        0,
        17,
        8,
        0,
        0x40,
        0,
        0x80,
        3,
        1,
        0x11,
        0,
        2,
        0x11,
        0,
        3,
        0x11,
        0,
        0xFF,
        0xD9,
      ]);
      await api.uploadLogo(jpeg);
      expect(await api.getLogo(), jpeg);
      await expectLater(
        api.uploadLogo(Uint8List.fromList([1, 2, 3])),
        throwsA(isA<ApiException>()),
      );
      await api.deleteLogo();
      expect(await api.getLogo(), isNull);

      final pdfUri = await api.quotePdfUri(
        saved['id'] as String,
        extra: {'margin_percent': '20'},
      );
      final pdf = await http.get(pdfUri); // no login header, only the ticket
      expect(pdf.statusCode, 200);
      expect(String.fromCharCodes(pdf.bodyBytes.take(5)), '%PDF-');
      expect(
        (await http.get(
          Uri.parse(
            pdfUri.toString().replaceAll(
              RegExp(r'ticket=[^&]+'),
              'ticket=bogus',
            ),
          ),
        )).statusCode,
        401,
      );

      // signing out ends the session
      await api.logout();
      await expectLater(api.listOrders(), throwsA(isA<ApiException>()));
    },
    skip: url == null ? 'set AMBER_LIVE_URL to run' : false,
  );
}

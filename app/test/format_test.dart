import 'package:amber_app/format.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('rounds half-up on decimal strings with no floating point', () {
    expect(fmt('112585.6'), '112,585.60');
    expect(fmt('215.66656'), '215.67');
    expect(fmt('0.005'), '0.01');
    expect(fmt('0.004'), '0.00');
    expect(fmt('1976.71168'), '1,976.71');
    expect(fmt('95.238095238095238095238095'), '95.24');
    expect(fmt('999.995'), '1,000.00');
    expect(fmt('12112.0'), '12,112.00');
  });

  test('handles zero, whole numbers, negatives and decimal places', () {
    expect(fmt('0'), '0.00');
    expect(fmt('700', dp: 0), '700');
    expect(fmt('1219.2', dp: 1, group: false), '1219.2');
    expect(fmt('-1.005'), '-1.01');
    expect(fmt('-0.001'), '0.00');
    expect(fmt('.5', dp: 0), '1');
  });

  test('large values stay exact (beyond double precision)', () {
    expect(fmt('12345678901234567890.125'), '12,345,678,901,234,567,890.13');
  });

  test('missing and non-numeric values are shown plainly', () {
    expect(fmt(null), '-');
    expect(fmt(''), '-');
    expect(fmt('abc'), 'abc');
  });
}

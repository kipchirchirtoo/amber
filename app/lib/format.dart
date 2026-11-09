/// Display formatting only. The app never does business arithmetic: every
/// number arrives from the Rust engine as an exact decimal STRING and is only
/// rounded for display here, using integer (BigInt) math so no float is involved.
String fmt(Object? value, {int dp = 2, bool group = true}) {
  if (value == null) return '-';
  var s = value.toString().trim();
  if (s.isEmpty) return '-';
  final negative = s.startsWith('-');
  if (negative) s = s.substring(1);
  final parts = s.split('.');
  final whole = parts[0].isEmpty ? '0' : parts[0];
  final frac = parts.length > 1 ? parts[1] : '';
  if (!RegExp(r'^\d+$').hasMatch(whole) ||
      (frac.isNotEmpty && !RegExp(r'^\d+$').hasMatch(frac))) {
    return value.toString(); // not a plain decimal: show as received
  }

  var scaled = BigInt.parse(whole + frac.padRight(dp, '0').substring(0, dp));
  if (frac.length > dp && frac.codeUnitAt(dp) >= 0x35) {
    scaled += BigInt.one; // half-up
  }

  var digits = scaled.toString().padLeft(dp + 1, '0');
  var intPart = digits.substring(0, digits.length - dp);
  final fracPart = digits.substring(digits.length - dp);
  if (group) {
    final b = StringBuffer();
    for (var i = 0; i < intPart.length; i++) {
      if (i > 0 && (intPart.length - i) % 3 == 0) b.write(',');
      b.write(intPart[i]);
    }
    intPart = b.toString();
  }
  final body = dp > 0 ? '$intPart.$fracPart' : intPart;
  return (negative && scaled != BigInt.zero ? '-' : '') + body;
}

String ksh(Object? v) => 'KSh ${fmt(v)}';

const profileLabels = {
  'top_track': 'Top track',
  'bottom_track': 'Bottom track',
  'side_track': 'Side track',
  'meeting_profile': 'Meeting profile',
  'locking_profile': 'Locking profile',
  'sash': 'Sash',
};

const fittingLabels = {
  'roller': 'Rollers',
  'crescent_lock': 'Crescent locks',
  'silver_screw': 'Silver screws',
  'black_mdf_screw': 'Black MDF screws',
};

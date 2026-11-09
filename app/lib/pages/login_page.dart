import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';

/// Shown until someone signs in. Accounts are made by an administrator.
class LoginPage extends StatefulWidget {
  const LoginPage({super.key, required this.controller});
  final AppController controller;

  @override
  State<LoginPage> createState() => _LoginPageState();
}

class _LoginPageState extends State<LoginPage> {
  late final _server = TextEditingController(
    text: widget.controller.api.baseUrl,
  );
  final _user = TextEditingController();
  final _pass = TextEditingController();
  String? _error;
  bool _busy = false;
  bool _showPassword = false;

  @override
  void initState() {
    super.initState();
    _error = widget.controller.signedOutReason;
  }

  @override
  void dispose() {
    _server.dispose();
    _user.dispose();
    _pass.dispose();
    super.dispose();
  }

  Future<void> _signIn() async {
    if (_busy) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await widget.controller.signIn(
        _server.text.trim(),
        _user.text.trim(),
        _pass.text,
      );
    } on ApiException catch (e) {
      if (mounted) setState(() => _error = e.message);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final t = Theme.of(context);
    return Scaffold(
      body: Center(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 380),
            child: AutofillGroup(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(
                    'Amber Stores Kenya',
                    style: t.textTheme.headlineSmall,
                    textAlign: TextAlign.center,
                  ),
                  Text(
                    'Sliding Window Calculator',
                    style: t.textTheme.titleMedium,
                    textAlign: TextAlign.center,
                  ),
                  const SizedBox(height: 28),
                  TextField(
                    controller: _user,
                    autofocus: true,
                    autofillHints: const [AutofillHints.username],
                    decoration: const InputDecoration(
                      labelText: 'Username',
                      border: OutlineInputBorder(),
                    ),
                    textInputAction: TextInputAction.next,
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _pass,
                    obscureText: !_showPassword,
                    autofillHints: const [AutofillHints.password],
                    decoration: InputDecoration(
                      labelText: 'Password',
                      border: const OutlineInputBorder(),
                      suffixIcon: IconButton(
                        tooltip: _showPassword
                            ? 'Hide password'
                            : 'Show password',
                        icon: Icon(
                          _showPassword
                              ? Icons.visibility_off
                              : Icons.visibility,
                        ),
                        onPressed: () => setState(() {
                          _showPassword = !_showPassword;
                        }),
                      ),
                    ),
                    onSubmitted: (_) => _signIn(),
                  ),
                  const SizedBox(height: 12),
                  TextField(
                    controller: _server,
                    decoration: const InputDecoration(
                      labelText: 'Server address',
                      border: OutlineInputBorder(),
                      isDense: true,
                    ),
                    onSubmitted: (_) => _signIn(),
                  ),
                  if (_error != null)
                    Padding(
                      padding: const EdgeInsets.only(top: 12),
                      child: Text(
                        _error!,
                        style: TextStyle(color: t.colorScheme.error),
                      ),
                    ),
                  const SizedBox(height: 16),
                  FilledButton(
                    onPressed: _busy ? null : _signIn,
                    child: Padding(
                      padding: const EdgeInsets.all(6),
                      child: Text(_busy ? 'Signing in...' : 'Sign in'),
                    ),
                  ),
                  const SizedBox(height: 12),
                  Text(
                    'Ask the administrator if you need an account or forgot your password.',
                    style: t.textTheme.bodySmall,
                    textAlign: TextAlign.center,
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

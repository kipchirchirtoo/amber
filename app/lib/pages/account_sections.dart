import 'package:flutter/material.dart';

import '../api.dart';
import '../draft.dart';

/// Who you are signed in as, and changing your own password.
class AccountSection extends StatefulWidget {
  const AccountSection({super.key, required this.controller});
  final AppController controller;

  @override
  State<AccountSection> createState() => _AccountSectionState();
}

class _AccountSectionState extends State<AccountSection> {
  final _current = TextEditingController();
  final _new = TextEditingController();
  final _again = TextEditingController();
  String? _msg;
  bool _ok = false;

  @override
  void dispose() {
    _current.dispose();
    _new.dispose();
    _again.dispose();
    super.dispose();
  }

  Future<void> _change() async {
    if (_new.text != _again.text) {
      setState(() {
        _msg = 'The two new passwords are not the same.';
        _ok = false;
      });
      return;
    }
    try {
      await widget.controller.api.changePassword(_current.text, _new.text);
      _current.clear();
      _new.clear();
      _again.clear();
      setState(() {
        _msg = 'Password changed. Other devices have been signed out.';
        _ok = true;
      });
    } on ApiException catch (e) {
      setState(() {
        _msg = e.message;
        _ok = false;
      });
    }
  }

  Widget _pw(TextEditingController c, String label) => SizedBox(
    width: 220,
    child: TextField(
      controller: c,
      obscureText: true,
      decoration: InputDecoration(
        labelText: label,
        isDense: true,
        border: const OutlineInputBorder(),
      ),
    ),
  );

  @override
  Widget build(BuildContext context) {
    final u = widget.controller.user!;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          '${u['display_name']} (${u['username']}), ${u['role'] == 'admin' ? 'administrator' : 'staff'}',
        ),
        const SizedBox(height: 12),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            _pw(_current, 'Current password'),
            _pw(_new, 'New password (8+ characters)'),
            _pw(_again, 'New password again'),
            FilledButton(
              onPressed: _change,
              child: const Text('Change password'),
            ),
          ],
        ),
        if (_msg != null)
          Padding(
            padding: const EdgeInsets.only(top: 8),
            child: Text(
              _msg!,
              style: _ok
                  ? null
                  : TextStyle(color: Theme.of(context).colorScheme.error),
            ),
          ),
      ],
    );
  }
}

/// Administrator only: add people, change roles, switch accounts off, reset passwords.
class UsersSection extends StatefulWidget {
  const UsersSection({super.key, required this.controller});
  final AppController controller;

  @override
  State<UsersSection> createState() => _UsersSectionState();
}

class _UsersSectionState extends State<UsersSection> {
  late Future<List<Json>> _users = widget.controller.api.listUsers();
  final _username = TextEditingController();
  final _display = TextEditingController();
  final _password = TextEditingController();
  String _role = 'staff';
  String? _error;

  ApiClient get api => widget.controller.api;

  @override
  void dispose() {
    _username.dispose();
    _display.dispose();
    _password.dispose();
    super.dispose();
  }

  void _reload() => setState(() {
    _users = api.listUsers();
  });

  Future<void> _run(Future<void> Function() job) async {
    try {
      await job();
      setState(() => _error = null);
      _reload();
    } on ApiException catch (e) {
      setState(() => _error = e.message);
    }
  }

  Future<void> _add() => _run(() async {
    await api.createUser(
      username: _username.text.trim(),
      displayName: _display.text.trim(),
      role: _role,
      password: _password.text,
    );
    _username.clear();
    _display.clear();
    _password.clear();
  });

  Future<void> _resetPassword(Json u) async {
    final c = TextEditingController();
    final pw = await showDialog<String>(
      context: context,
      builder: (ctx) => AlertDialog(
        title: Text('New password for ${u['username']}'),
        content: TextField(
          controller: c,
          autofocus: true,
          decoration: const InputDecoration(
            labelText: 'New password (8+ characters)',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(ctx),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(ctx, c.text),
            child: const Text('Set password'),
          ),
        ],
      ),
    );
    c.dispose();
    if (pw == null || pw.isEmpty) return;
    await _run(
      () async => api.updateUser(u['id'] as String, {'new_password': pw}),
    );
  }

  @override
  Widget build(BuildContext context) {
    final me = widget.controller.user!['id'];
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        FutureBuilder<List<Json>>(
          future: _users,
          builder: (context, snap) {
            if (snap.connectionState != ConnectionState.done) {
              return const LinearProgressIndicator();
            }
            if (snap.hasError) return Text('${snap.error}');
            return Column(
              children: [
                for (final u in snap.data!)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    dense: true,
                    title: Text(
                      '${u['display_name']} (${u['username']})${u['id'] == me ? '  - you' : ''}',
                    ),
                    subtitle: Text(
                      '${u['role'] == 'admin' ? 'Administrator' : 'Staff'}${u['active'] == true ? '' : '  |  ACCOUNT OFF'}',
                    ),
                    trailing: Wrap(
                      spacing: 4,
                      children: [
                        TextButton(
                          onPressed: () => _resetPassword(u),
                          child: const Text('Reset password'),
                        ),
                        TextButton(
                          onPressed: () => _run(
                            () async => api.updateUser(u['id'] as String, {
                              'role': u['role'] == 'admin' ? 'staff' : 'admin',
                            }),
                          ),
                          child: Text(
                            u['role'] == 'admin' ? 'Make staff' : 'Make admin',
                          ),
                        ),
                        TextButton(
                          onPressed: () => _run(
                            () async => api.updateUser(u['id'] as String, {
                              'active': u['active'] != true,
                            }),
                          ),
                          child: Text(
                            u['active'] == true ? 'Switch off' : 'Switch on',
                          ),
                        ),
                      ],
                    ),
                  ),
              ],
            );
          },
        ),
        const SizedBox(height: 8),
        Text('Add a person', style: Theme.of(context).textTheme.titleSmall),
        const SizedBox(height: 8),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            SizedBox(
              width: 160,
              child: TextField(
                controller: _username,
                decoration: const InputDecoration(
                  labelText: 'Username',
                  isDense: true,
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            SizedBox(
              width: 180,
              child: TextField(
                controller: _display,
                decoration: const InputDecoration(
                  labelText: 'Name shown',
                  isDense: true,
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            SizedBox(
              width: 200,
              child: TextField(
                controller: _password,
                decoration: const InputDecoration(
                  labelText: 'First password (8+)',
                  isDense: true,
                  border: OutlineInputBorder(),
                ),
              ),
            ),
            DropdownButton<String>(
              value: _role,
              items: const [
                DropdownMenuItem(value: 'staff', child: Text('Staff')),
                DropdownMenuItem(value: 'admin', child: Text('Administrator')),
              ],
              onChanged: (v) => setState(() => _role = v!),
            ),
            FilledButton.icon(
              onPressed: _add,
              icon: const Icon(Icons.person_add),
              label: const Text('Add'),
            ),
          ],
        ),
        Padding(
          padding: const EdgeInsets.only(top: 6),
          child: Text(
            'Usernames use lower-case letters, digits, . _ - or @ . Staff can calculate, save orders, keep stock and make quotations. Only administrators change prices, users and the letterhead.',
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
      ],
    );
  }
}

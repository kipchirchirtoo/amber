import 'package:flutter/material.dart';

import '../draft.dart';
import '../theme.dart';
import 'result_view.dart' show StatusChip;
import '../widgets.dart';
import 'account_sections.dart';

/// Your own account: who you are signed in as, your role, and your password.
class ProfilePage extends StatelessWidget {
  const ProfilePage({super.key, required this.controller});
  final AppController controller;

  @override
  Widget build(BuildContext context) {
    final u = controller.user;
    if (u == null) return const SizedBox.shrink();
    final admin = controller.isAdmin;
    final name = controller.userLabel;
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        const PageHeader(
          'Profile settings',
          subtitle: 'Your account on this system',
        ),
        Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 860),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Card(
                  child: Padding(
                    padding: const EdgeInsets.all(20),
                    child: Row(
                      children: [
                        UserAvatar(name, radius: 36),
                        const SizedBox(width: 20),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text(
                                name,
                                style: const TextStyle(
                                  fontSize: 22,
                                  fontWeight: FontWeight.w700,
                                ),
                              ),
                              const SizedBox(height: 2),
                              Text(
                                '${u['username']}',
                                style: Theme.of(context).textTheme.bodySmall,
                              ),
                              const SizedBox(height: 8),
                              StatusChip(
                                admin ? 'Administrator' : 'Staff',
                                ok: true,
                              ),
                            ],
                          ),
                        ),
                        MiniButton(
                          'Sign out',
                          icon: Icons.logout,
                          onPressed: controller.signOut,
                        ),
                      ],
                    ),
                  ),
                ),
                const SizedBox(height: 14),
                AppCard(
                  title: 'Account details',
                  icon: Icons.badge_outlined,
                  child: KvGrid([
                    ('Display name', KvGrid.text(name, bold: true)),
                    ('Username', KvGrid.text('${u['username']}')),
                    ('Role', KvGrid.text(admin ? 'Administrator' : 'Staff')),
                    ('Server', KvGrid.text(controller.api.baseUrl)),
                  ]),
                ),
                const SizedBox(height: 14),
                AppCard(
                  title: 'What you can do',
                  icon: Icons.verified_user_outlined,
                  child: Text(
                    admin
                        ? 'Calculate and save orders, keep stock, make quotations, and as an administrator change prices, users and the quotation letterhead (Settings).'
                        : 'Calculate and save orders, keep stock and make quotations. Only administrators change prices, users and the letterhead.',
                  ),
                ),
                const SizedBox(height: 14),
                AppCard(
                  title: 'Change password',
                  icon: Icons.lock_outline,
                  child: AccountSection(controller: controller),
                ),
                const SizedBox(height: 8),
                const Text(
                  'Display name and role are changed by an administrator in Settings, under Users.',
                  style: TextStyle(fontSize: 12, color: Brand.muted),
                ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

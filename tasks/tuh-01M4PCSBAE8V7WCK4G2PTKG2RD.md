# Permissions: Settings watches them live; ask for Input Monitoring so EasyVox appears in its list

`tuh-01M4PCSBAE8V7WCK4G2PTKG2RD`

- **Status:** open — ready
- **Priority:** none
- **Labels:** `app` `permissions`
- **Created:** 2026-10-11 21:09 UTC by `brandon`

## Description

Brandon, testing the restyled build: 'The Accessibility setting doesn't detect when I grant the permission' and 'When I try to turn on the Input Monitoring setting, EasyVox doesn't show in the list'. Settings › Permissions read the permissions only when it redrew, and EasyVox never asked macOS for Input Monitoring (only preflighted), so it never entered that list. Fix: Settings watches the permissions like the setup window (shared code), Input Monitoring is asked for (prompt, then System Settings), and right ⌥ starts working once it's allowed.

## History

_No activity yet._

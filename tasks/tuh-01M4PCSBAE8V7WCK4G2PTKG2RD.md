# Permissions: Settings watches them live; ask for Input Monitoring so EasyVox appears in its list

`tuh-01M4PCSBAE8V7WCK4G2PTKG2RD`

- **Status:** done
- **Priority:** none
- **Labels:** `app` `permissions`
- **Created:** 2026-10-11 21:09 UTC by `brandon`

## Description

Brandon, testing the restyled build: 'The Accessibility setting doesn't detect when I grant the permission' and 'When I try to turn on the Input Monitoring setting, EasyVox doesn't show in the list'. Done in worktree agent-a5c0f787750044b9e (uncommitted): Settings › Permissions now keeps the same permission watch as setup (new permissions.rs: reads on open, on coming forward, every second while one is missing; stops when all are allowed or the window closes; a change also reaches the menu bar). Input Monitoring is asked for with IOHIDRequestAccess (turning right ⌥ on, or Allow… on its row), which puts EasyVox in the list; read with IOHIDCheckAccess, because SkyLight caches CGRequestListenEventAccess's answer for the life of the process and every listen-only tap makes that request. The right-⌥ tap is installed once the permission is allowed, without a relaunch (to be confirmed by hand). DESIGN.md, unit tests and a self-test check updated.

## History

### 2026-10-11 21:17 UTC — edit by `brandon`

description edited · status open→done

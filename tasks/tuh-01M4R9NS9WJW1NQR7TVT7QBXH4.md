# Self-test flake: Settings › Permissions counted 1 read while all allowed (expects 0)

`tuh-01M4R9NS9WJW1NQR7TVT7QBXH4`

- **Status:** inbox — untriaged capture
- **Priority:** none
- **Created:** 2026-10-12 14:53 UTC by `brandon`

## Description

12 Oct, hardened release build: 1 failure in 6 runs. reads_in_3s_all_allowed was 1, reads_in_3s_one_missing 3. Likely a one-second watch tick already scheduled from the 'Microphone denied' case firing after the switch to all allowed. Not the hardened runtime: no AMFI or library-validation denials in the log. selftest.rs ~1095.

Cause: Watch::new observes window activation and rereads on every activation (permissions.rs ~90). If the Settings window activates inside the 3-second all-allowed window, the count is 1. Fix the test, not the app: let activation settle before measuring, or count activation reads separately.

## History

### 2026-10-12 14:57 UTC — edit by `brandon`

description edited

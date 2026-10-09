# Desktop notifications (0.4.0)

Open **Settings** in the header. Appearance choices remain there, followed by
**Desktop notifications**. All preferences use the existing local `state.json`
and apply immediately to subsequent delivery decisions, including convoys already
running. Old settings files and partially specified notification settings receive
these defaults:

| Preference | Default |
| --- | --- |
| Enable desktop notifications | On |
| Successful completion | On |
| Failure | On |
| Cancellation | Off |
| Review needed | On |
| Suppress while foreground | On |

There is one delivery attempt per eligible convoy, after every repository job is
terminal. The highest-priority condition selects the filter: **failure →
cancellation → review needed → success**. Disabling that filter does not fall
through to another outcome. Success and review are separate concepts: successful
execution can leave changes needing review. Unknown Git results also use the review
filter and explicitly say they could not be checked. A failure or cancellation
alert includes execution counts and any already known review information without
waiting for further inspection.

Review uses Git observations, never agent prose. Isolated results use the runner's
session observation against the saved base commit. Direct results use cached Review
statistics when available, otherwise a background, read-only Git status. Direct
observations describe the current mutable repository, including pre-existing
changes; they cannot attribute changes to an agent. Inspection errors are not
reported as unchanged. Notifications do not validate ownership or authorize any
result action. Apply/Discard and recovery keep their existing safety checks.

While the root window is focused, success, cancellation and review alerts are
suppressed by default. Enabled failure alerts still notify. A minimized window is
not considered foreground, even if its last focus flag was true. Focus and filters
are checked when the completed result is ready for delivery. Suppressed, disabled
or failed alerts are never retried on blur, preference changes or restart.

Notifications contain convoy IDs and counts, not task text, repository names,
paths, source code or agent output. Clicks select the existing convoy Review view,
restore the window, then request focus on the next event-loop pass. Multiple alerts
keep separate IDs. A removed convoy produces a “no longer in history” notice;
unavailable retained files still open their existing diagnostics. Assessment can
finish after history removal without retaining/pinning that history.

## Platforms and permissions

- **Linux / GNOME / Wayland:** `notify-rust` 4.18.2 uses the session D-Bus desktop
  notification service and the `codeconvoy` desktop identity. Its asynchronous
  action listener stays in this process. The service must support actions for
  clicks to route. egui's focus request has no effect on Wayland: the correct
  convoy is selected, but you may need to activate CodeConvoy from the desktop.
  No X11-only focus workaround or activation daemon is used.
- **macOS:** `mac-usernotifications` 0.3.1 uses modern `UNUserNotificationCenter`
  with asynchronous authorization and response callbacks. Use the packaged,
  signed CodeConvoy `.app`; the existing ad-hoc signature is supported by the
  library. A bare `cargo run` executable has no bundle identity and cannot deliver
  these notifications. The first eligible delivery requests OS permission;
  denial remains under System Settings → Notifications. The library's delegate
  works with eframe's main run loop and does not replace the application delegate.
- **Windows 10/11:** `tauri-winrt-notification` 0.8.1 supplies standalone native
  WinRT toasts and in-process activation callbacks. It is a native API wrapper;
  it adds no Tauri framework, WebView or web frontend. Both installer and portable
  builds register their display name under the current user's
  `Software\Classes\AppUserModelId\io.github.chrislauinger77.code-convoy` key.
  No elevation, service or executable activation registration is used. Windows
  can restrict foreground activation. Notification Center callbacks after banner
  expiry depend on Windows' in-process toast lifetime and need native validation.

System permissions, Focus/Do Not Disturb and desktop notification preferences stay
in control. CodeConvoy does not request critical/time-sensitive alerts or bypass
those preferences. Delivery failures appear under **Settings (!)** and do not
change agent outcomes or stop other convoys. Dismiss the diagnostic there after
checking your OS settings. There is no automatic retry or delivery receipt from
the operating system guaranteeing that a banner was visible.

Clicks are supported for the current application process. CodeConvoy does not
register cold-start routing; old OS notifications cannot reliably select a run
after exit/restart. Closing cancels notification tasks without waiting for user
interaction. A crash, exit or denied OS permission can prevent delivery; exactly
once means one eligible submission attempt, not guaranteed OS presentation.

## Library decision and sources

The maintained [notify-rust](https://github.com/hoodie/notify-rust) family provides
all three native transports. Linux uses its asynchronous API with the same zbus
async-io configuration already used by accessibility. Enabling zbus's Tokio
feature globally would also change those other consumers. On macOS and Windows
we call its underlying libraries directly because the common response API blocks:
[mac-usernotifications](https://github.com/hoodie/mac-usernotifications) exposes a
cancellable response future and
[winrt-notification](https://github.com/tauri-apps/winrt-notification) exposes native
callbacks. `windows-registry` provides safe, per-user portable-app registration
following that library's unpackaged-app example. The adapter is isolated in
`src/notifications/native.rs`.

The exact downloaded crate sources were inspected, along with egui 0.36.2's
`ViewportInfo` and `ViewportCommand` APIs. See
[egui viewport commands](https://docs.rs/egui/0.36.2/egui/enum.ViewportCommand.html)
for focus restrictions. Source verification is not native runtime validation.

## Validation and native smoke checklist

Automated policy tests use a mock backend. They cover defaults and backward
compatibility, Store persistence, every filter/focus combination, outcome priority,
partial/mixed completion, cancellation, changed/unknown/resolved results,
exactly-once lifecycle tracking, restart silence, multiple stable click IDs,
removed/unavailable history, backend errors/panics, and listener cancellation.
Real temporary Git repositories verify clean, tracked, staged-alternative and
untracked Direct observations without changing HEAD or the real index. Headless
egui tests verify focus/minimized inputs, restore-before-focus commands and existing
Review navigation. No authenticated agent is needed.

Native notification smoke tests remain **unperformed** on all three platforms.
Before publishing 0.4.0, use disposable repositories and fixture agents to check:

- Linux GNOME/Wayland and X11: success while unfocused/minimized; silence while
  focused; failure while focused; one alert for a multi-repository mixed run;
  independent alerts and click routing for two concurrent convoys; manual desktop
  activation if Wayland prevents raising the window; absent D-Bus service/actions.
- macOS packaged `.app` on Apple Silicon and Intel: permission grant/deny, Focus
  mode, notification body and Open convoy button, two simultaneous completions,
  minimized restore, clear-all/dismiss, About and Quit still working. Bare binary
  failure should remain a notification diagnostic without affecting execution.
- Windows 10 and 11 installed and portable: CodeConvoy sender identity, permission
  settings/Do Not Disturb, body click while another convoy is selected, banner and
  Notification Center click behavior, minimized restore and foreground restrictions.
- Everywhere: toggle each filter during a run, remove history before clicking,
  leave an isolated result unavailable, quit with visible notifications, restart
  terminal/interrupted history, and verify that no completion alerts replay.

Native macOS/Windows builds, package candidate installation and the above desktop
checks are release gates still requiring their respective hosts. Linux source
validation results are recorded in [release notes](release-notes.md).

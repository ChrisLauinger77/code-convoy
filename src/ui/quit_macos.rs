//! AppKit's terminate: bypasses winit's cancellable window-close event.
//! Own the application subclass, leaving winit's delegate and event loop intact.
use eframe::egui;
use objc2::{ClassType, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::NSApplication;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol};
use std::cell::RefCell;

thread_local! {
    static CONTEXT: RefCell<Option<(egui::Context, std::sync::mpsc::Sender<super::Message>)>> = const { RefCell::new(None) };
}

define_class!(
    // SAFETY: NSApplication supports subclassing. No ivars or initialization
    // overrides; AppKit and winit retain ownership of the native lifecycle.
    #[unsafe(super = NSApplication)]
    #[name = "CodeConvoyApplication"]
    #[thread_kind = MainThreadOnly]
    struct ConvoyApplication;

    impl ConvoyApplication {
        // SAFETY: Matches NSApplication's main-thread terminate: action signature.
        #[unsafe(method(terminate:))]
        fn request_termination(&self, sender: Option<&AnyObject>) {
            let ctx = CONTEXT.with(|slot| slot.borrow().clone());
            if let Some((ctx, tx)) = ctx {
                // Cmd+Q, the application menu and Dock Quit all reach this action.
                // Never call super here: eframe must first ask the shared UI.
                let _ = tx.send(super::Message::DesktopAction(crate::desktop::Action::Quit));
                ctx.request_repaint();
            } else {
                // No UI/jobs yet (startup failure or exit before initialization).
                unsafe { msg_send![super(self), terminate: sender] }
            }
        }
    }
);

/// Must run on the main thread, before eframe or any native dialog creates NSApp.
pub fn init_native_application() -> anyhow::Result<()> {
    let _mtm = MainThreadMarker::new().ok_or_else(|| anyhow::anyhow!("Not on the main thread."))?;
    // SAFETY: Inherited sharedApplication creates/returns AppKit's singleton. Use
    // the base return type and check it, since another library may have made it.
    let app: Retained<NSApplication> =
        unsafe { msg_send![ConvoyApplication::class(), sharedApplication] };
    anyhow::ensure!(
        app.isKindOfClass(ConvoyApplication::class()),
        "Cannot install the application quit confirmation."
    );
    Ok(())
}

pub(super) fn connect(ctx: &egui::Context, tx: std::sync::mpsc::Sender<super::Message>) {
    CONTEXT.with(|slot| *slot.borrow_mut() = Some((ctx.clone(), tx)));
}

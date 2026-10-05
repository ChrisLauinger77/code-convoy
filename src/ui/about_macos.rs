//! Keep AppKit's standard About panel, supplying only its two version fields.

use objc2::{MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject, sel};
use objc2_app_kit::{
    NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionVersion, NSApplication, NSMenuItem,
};
use objc2_foundation::{MainThreadMarker, NSDictionary, NSObject, NSString};

define_class!(
    // SAFETY: NSObject has no subclassing requirements. The target lives and is
    // invoked only on the main thread, and has no custom Objective-C ivars.
    #[unsafe(super = NSObject)]
    #[name = "CodeConvoyAboutTarget"]
    #[thread_kind = MainThreadOnly]
    struct AboutTarget;

    impl AboutTarget {
        // SAFETY: This is the standard menu action signature (one object argument).
        #[unsafe(method(showCodeConvoyAbout:))]
        fn show_about(&self, _sender: Option<&AnyObject>) {
            let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
            // An absent key would fall back to the numeric CFBundleVersion.
            // An explicit empty string suppresses the parenthesized build value.
            let revision = NSString::from_str(
                super::about::valid_revision(option_env!("CODECONVOY_GIT_REV")).unwrap_or(""),
            );
            // SAFETY: These are AppKit's NSString keys; both values are NSStrings
            // as required by the standard About panel. All other defaults,
            // including the application icon, remain untouched.
            unsafe {
                let options = NSDictionary::<NSString, AnyObject>::from_slices(
                    &[NSAboutPanelOptionApplicationVersion, NSAboutPanelOptionVersion],
                    &[&version, &revision],
                );
                NSApplication::sharedApplication(self.mtm())
                    .orderFrontStandardAboutPanelWithOptions(&options);
            }
        }
    }
);

pub(super) struct NativeAbout {
    item: Retained<NSMenuItem>,
    // NSMenuItem's target is weak, so retain it for the lifetime of the app.
    _target: Retained<AboutTarget>,
}

impl NativeAbout {
    pub(super) fn install() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let menu = NSApplication::sharedApplication(mtm).mainMenu()?;
        for menu_item in menu.itemArray() {
            let Some(submenu) = menu_item.submenu() else {
                continue;
            };
            for item in submenu.itemArray() {
                if item.action() == Some(sel!(orderFrontStandardAboutPanel:)) {
                    // SAFETY: NSObject's init has this signature. The target
                    // implements the action and is retained alongside the item.
                    let target: Retained<AboutTarget> =
                        unsafe { msg_send![AboutTarget::alloc(mtm), init] };
                    unsafe {
                        item.setTarget(Some(&target));
                        item.setAction(Some(sel!(showCodeConvoyAbout:)));
                    }
                    return Some(Self {
                        item,
                        _target: target,
                    });
                }
            }
        }
        None
    }
}

impl Drop for NativeAbout {
    fn drop(&mut self) {
        // SAFETY: Restore the standard responder-chain action before releasing
        // our target. NativeAbout is main-thread-only through its retained objects.
        unsafe {
            self.item.setTarget(None);
            self.item
                .setAction(Some(sel!(orderFrontStandardAboutPanel:)));
        }
    }
}

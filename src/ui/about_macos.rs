//! Native application-menu branding and the standard About panel's project link.

use objc2::{
    AnyThread, MainThreadOnly, define_class, msg_send, rc::Retained, runtime::AnyObject, sel,
};
use objc2_app_kit::{
    NSAboutPanelOptionApplicationName, NSAboutPanelOptionApplicationVersion,
    NSAboutPanelOptionCredits, NSAboutPanelOptionVersion, NSApplication, NSFont,
    NSFontAttributeName, NSLinkAttributeName, NSMenuItem, NSMutableParagraphStyle,
    NSParagraphStyleAttributeName, NSTextAlignment,
};
use objc2_foundation::{
    MainThreadMarker, NSAttributedString, NSDictionary, NSObject, NSString, ns_string,
};

fn repository_link() -> Retained<NSAttributedString> {
    let label = NSString::from_str("GitHub repository");
    let url = NSString::from_str(super::about::REPOSITORY_URL);
    let paragraph = NSMutableParagraphStyle::new();
    paragraph.setAlignment(NSTextAlignment::Center);
    let font = NSFont::systemFontOfSize(NSFont::smallSystemFontSize());
    // SAFETY: AppKit defines link values as NSString/NSURL, paragraph styles as
    // NSParagraphStyle, and fonts as NSFont. The attributed string retains them.
    unsafe {
        let attributes = NSDictionary::<NSString, AnyObject>::from_slices(
            &[
                NSLinkAttributeName,
                NSParagraphStyleAttributeName,
                NSFontAttributeName,
            ],
            &[&url, &paragraph, &font],
        );
        NSAttributedString::initWithString_attributes(
            NSAttributedString::alloc(),
            &label,
            Some(&attributes),
        )
    }
}

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
            let name = NSString::from_str(super::about::APPLICATION_NAME);
            let version = NSString::from_str(env!("CARGO_PKG_VERSION"));
            // An absent key would fall back to the numeric CFBundleVersion.
            // An explicit empty string suppresses the parenthesized build value.
            let revision = NSString::from_str(
                super::about::valid_revision(option_env!("CODECONVOY_GIT_REV")).unwrap_or(""),
            );
            let credits = repository_link();
            // SAFETY: AppKit expects NSStrings for name/version/copyright and
            // an NSAttributedString for Credits. An explicit empty Copyright
            // suppresses the bundle fallback; the native icon stays unchanged.
            unsafe {
                let options = NSDictionary::<NSString, AnyObject>::from_slices(
                    &[
                        NSAboutPanelOptionApplicationName,
                        NSAboutPanelOptionApplicationVersion,
                        NSAboutPanelOptionVersion,
                        NSAboutPanelOptionCredits,
                        ns_string!("Copyright"),
                    ],
                    &[&name, &version, &revision, &credits, ns_string!("")],
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
            let mut about = None;
            for item in submenu.itemArray() {
                let action = item.action();
                // winit builds these labels from the lowercase executable name.
                // Match actions, preserving their targets and keyboard shortcuts.
                let label = if action == Some(sel!(orderFrontStandardAboutPanel:)) {
                    Some("About")
                } else if action == Some(sel!(hide:)) {
                    Some("Hide")
                } else if action == Some(sel!(terminate:)) {
                    Some("Quit")
                } else {
                    None
                };
                if let Some(label) = label {
                    item.setTitle(&NSString::from_str(&format!(
                        "{label} {}",
                        super::about::APPLICATION_NAME
                    )));
                }
                if action == Some(sel!(orderFrontStandardAboutPanel:)) {
                    about = Some(item);
                }
            }
            if let Some(item) = about {
                let name = NSString::from_str(super::about::APPLICATION_NAME);
                menu_item.setTitle(&name);
                submenu.setTitle(&name);
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

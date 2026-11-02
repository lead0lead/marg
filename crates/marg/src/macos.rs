//! macOS "Open With": Finder, the Dock and other apps hand files over as
//! an `odoc` Apple Event, not in argv, and winit ignores it (concept §11,
//! "macOS steps"). marg installs its own handler when the app is about to
//! finish launching (earlier, AppKit would replace it with its default;
//! later, the files of the launch event would be missed).

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send, sel};
use objc2_app_kit::NSApplicationWillFinishLaunchingNotification;
use objc2_foundation::{
    NSAppleEventDescriptor, NSAppleEventManager, NSNotification, NSNotificationCenter,
    NSObjectProtocol,
};

/// Four-character codes from the Apple Event headers.
const fn code(c: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*c)
}
const CORE_EVENT_CLASS: u32 = code(b"aevt");
const OPEN_DOCUMENTS: u32 = code(b"odoc");
const DIRECT_OBJECT: u32 = code(b"----");

struct Ivars {
    sender: Sender<Vec<PathBuf>>,
    wake: Box<dyn Fn()>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "MargOpenHandler"]
    #[ivars = Ivars]
    struct OpenHandler;

    impl OpenHandler {
        #[unsafe(method(willFinishLaunching:))]
        fn will_finish_launching(&self, _note: &NSNotification) {
            let manager = NSAppleEventManager::sharedAppleEventManager();
            // SAFETY: the selector below is defined on this class with the
            // signature AppKit calls; `self` lives for the whole process.
            unsafe {
                let _: () = msg_send![
                    &manager,
                    setEventHandler: self,
                    andSelector: sel!(handleOpenDocuments:withReplyEvent:),
                    forEventClass: CORE_EVENT_CLASS,
                    andEventID: OPEN_DOCUMENTS,
                ];
            }
        }

        #[unsafe(method(handleOpenDocuments:withReplyEvent:))]
        fn handle_open_documents(
            &self,
            event: &NSAppleEventDescriptor,
            _reply: &NSAppleEventDescriptor,
        ) {
            let paths = file_paths(event);
            log::info!("opened from outside: {paths:?}");
            if !paths.is_empty() && self.ivars().sender.send(paths).is_ok() {
                (self.ivars().wake)();
            }
        }
    }

    unsafe impl NSObjectProtocol for OpenHandler {}
);

/// The files of an `odoc` event: a list of file URLs.
fn file_paths(event: &NSAppleEventDescriptor) -> Vec<PathBuf> {
    // SAFETY: a plain getter taking a four-character code.
    let list: Option<Retained<NSAppleEventDescriptor>> =
        unsafe { msg_send![event, paramDescriptorForKeyword: DIRECT_OBJECT] };
    let Some(list) = list else {
        return Vec::new();
    };
    // Descriptor lists count from 1.
    (1..=list.numberOfItems())
        .filter_map(|i| list.descriptorAtIndex(i))
        .filter_map(|d| d.fileURLValue())
        .filter_map(|url| url.to_file_path())
        .collect()
}

/// Install the handler; call on the main thread before the event loop
/// starts. Files arrive on the returned channel, and `wake` is called
/// after each event.
pub fn install_open_handler(wake: impl Fn() + 'static) -> Receiver<Vec<PathBuf>> {
    let (sender, receiver) = mpsc::channel();
    let this = OpenHandler::alloc().set_ivars(Ivars {
        sender,
        wake: Box::new(wake),
    });
    // SAFETY: `init` of NSObject.
    let handler: Retained<OpenHandler> = unsafe { msg_send![super(this), init] };
    let center = NSNotificationCenter::defaultCenter();
    // SAFETY: the selector is defined on the handler and takes the
    // notification; the handler is never freed (below).
    unsafe {
        center.addObserver_selector_name_object(
            &handler,
            sel!(willFinishLaunching:),
            Some(NSApplicationWillFinishLaunchingNotification),
            None::<&AnyObject>,
        );
    }
    // Neither the notification center nor the event manager keeps it
    // alive; it is needed until the process exits.
    std::mem::forget(handler);
    receiver
}

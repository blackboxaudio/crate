//! iOS launch-screen overlay: bridges the gap between the system launch screen and the
//! webview's first contentful paint.
//!
//! iOS starts crossfading the launch screen into the app's first rendered frame as soon as the
//! view hierarchy commits — long before WKWebView has painted the page. Without this overlay the
//! crossfade lands on a blank white webview, so the launch logo visibly washes out and then pops
//! back once the HTML splash arrives. The fix: re-instantiate the launch storyboard's own view
//! (pixel-identical by construction, dark-mode aware via its named colors) and float it above the
//! webview until the frontend reports the web splash has painted (`dismiss_native_splash`), then
//! fade it out — which doubles as the wordmark/version fade-in, since those are the only pixels
//! that differ underneath.
//!
//! RECONCILE on device: UIKit has no objc2 bindings in this crate, so every UIKit selector below
//! is runtime-looked-up (`AnyClass::get` + `msg_send!`, the pattern proven in `commands/share.rs`
//! and `media_controls/ios/now_playing.rs`).

use std::cell::RefCell;

use block2::RcBlock;
use objc2::{
    encode::{Encode, Encoding},
    msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, Bool},
};
use objc2_foundation::NSString;
use tauri::{AppHandle, Manager};

/// How long the overlay may outlive the frontend before force-dismissing. Generous enough for a
/// cold dev-server load; short enough that a crashed frontend doesn't hide its error state forever.
const FAILSAFE_SECS: u64 = 8;

// CGGeometry structs, declared locally instead of pulling in objc2-core-foundation for three
// types. CGFloat is f64 on every Tauri-supported iOS target (arm64/arm64-sim).
#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}
// SAFETY: matches the ObjC @encode of CGPoint on 64-bit iOS.
unsafe impl Encode for CGPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[f64::ENCODING, f64::ENCODING]);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGSize {
    width: f64,
    height: f64,
}
// SAFETY: matches the ObjC @encode of CGSize on 64-bit iOS.
unsafe impl Encode for CGSize {
    const ENCODING: Encoding = Encoding::Struct("CGSize", &[f64::ENCODING, f64::ENCODING]);
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}
// SAFETY: matches the ObjC @encode of CGRect on 64-bit iOS.
unsafe impl Encode for CGRect {
    const ENCODING: Encoding = Encoding::Struct("CGRect", &[CGPoint::ENCODING, CGSize::ENCODING]);
}

thread_local! {
    /// The live overlay view. Main-thread only (all UIKit messaging happens there), retained so
    /// `dismiss` can animate + remove it; `None` once dismissed (making dismissal idempotent).
    static OVERLAY: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
}

/// Instantiate the launch storyboard's view and float it above the main window's WKWebView.
/// Call from `.setup()` (inside `didFinishLaunchingWithOptions`) so it's in place before the
/// system launch-screen crossfade begins. Best-effort: any missing piece just logs and skips.
pub fn attach(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        log::warn!("ios_splash: no main window to attach to");
        return;
    };
    let attached = window.with_webview(|webview| {
        // SAFETY: `with_webview` runs on the main thread; every object is nil-checked before use.
        unsafe {
            let wk = webview.inner() as *mut AnyObject;
            if wk.is_null() {
                log::warn!("ios_splash: null WKWebView");
                return;
            }
            // Prefer the webview's superview (the root VC's view) so the overlay stacks above the
            // whole webview; fall back to the webview itself if it isn't in a hierarchy yet.
            let superview: Option<Retained<AnyObject>> = msg_send![&*wk, superview];
            let parent: Retained<AnyObject> = match superview {
                Some(s) => s,
                None => Retained::retain(wk).expect("non-null WKWebView"),
            };

            let Some(sb_class) = AnyClass::get(c"UIStoryboard") else {
                return;
            };
            let name = NSString::from_str("LaunchScreen");
            let storyboard: Option<Retained<AnyObject>> =
                msg_send![sb_class, storyboardWithName: &*name, bundle: Option::<&AnyObject>::None];
            let Some(storyboard) = storyboard else {
                log::warn!("ios_splash: LaunchScreen storyboard not found");
                return;
            };
            let vc: Option<Retained<AnyObject>> =
                msg_send![&*storyboard, instantiateInitialViewController];
            let Some(vc) = vc else {
                log::warn!("ios_splash: launch storyboard has no initial view controller");
                return;
            };
            let view: Option<Retained<AnyObject>> = msg_send![&*vc, view];
            let Some(view) = view else { return };

            let bounds: CGRect = msg_send![&*parent, bounds];
            let _: () = msg_send![&*view, setFrame: bounds];
            // UIViewAutoresizingFlexibleWidth | UIViewAutoresizingFlexibleHeight
            let _: () = msg_send![&*view, setAutoresizingMask: ((1usize << 1) | (1usize << 4))];
            let _: () = msg_send![&*parent, addSubview: &*view];

            OVERLAY.set(Some(view));
        }
    });
    if let Err(e) = attached {
        log::warn!("ios_splash: with_webview failed: {e}");
        return;
    }

    // Failsafe: if the frontend never reports in (JS crash, unreachable dev server), reveal the
    // webview anyway rather than hiding whatever error state it's in. `dismiss` is idempotent.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(FAILSAFE_SECS)).await;
        dismiss(&handle);
    });
}

/// Fade the overlay out and remove it. Idempotent; safe to call from any thread.
pub fn dismiss(app: &AppHandle) {
    let _ = app.run_on_main_thread(|| {
        let Some(overlay) = OVERLAY.take() else {
            return;
        };
        // SAFETY: main thread (run_on_main_thread); the overlay is a valid retained UIView.
        unsafe {
            let Some(view_class) = AnyClass::get(c"UIView") else {
                let _: () = msg_send![&*overlay, removeFromSuperview];
                return;
            };
            // The fade is what makes the wordmark + version appear to ease in: the logo pixels are
            // identical above and below the overlay, so only the text region visibly changes.
            let anim_view = overlay.clone();
            let animations: RcBlock<dyn Fn()> = RcBlock::new(move || {
                let _: () = msg_send![&*anim_view, setAlpha: 0.0f64];
            });
            let done_view = overlay.clone();
            let completion: RcBlock<dyn Fn(Bool)> = RcBlock::new(move |_finished: Bool| {
                let _: () = msg_send![&*done_view, removeFromSuperview];
            });
            let _: () = msg_send![
                view_class,
                animateWithDuration: 0.25f64,
                animations: &*animations,
                completion: &*completion
            ];
        }
    });
}

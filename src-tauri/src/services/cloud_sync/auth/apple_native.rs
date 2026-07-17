//! Native iOS **Sign in with Apple** (App Store Review Guideline 4.8).
//!
//! Drives `ASAuthorizationController` / `ASAuthorizationAppleIDProvider`
//! (AuthenticationServices.framework) via objc2 to obtain an Apple **identity token** (a JWT),
//! which the caller then exchanges into Firebase through Identity Toolkit `signInWithIdp`
//! (`providerId = apple.com`) — the same exchange Google uses, plus the OIDC nonce. Crate never
//! sees the user's Apple password.
//!
//! ## Nonce
//! Apple copies the string set on `ASAuthorizationAppleIDRequest.nonce` **verbatim** into the
//! `nonce` claim of the returned identity token. Firebase, given the **raw** nonce in its
//! `postBody`, re-hashes it and compares `SHA256(raw)` to that claim. Therefore we set
//! `request.nonce = hex(SHA256(raw_nonce))` and return the **raw** nonce for the Firebase call
//! (see [`super::finish_sign_in`]). Swapping the two produces Firebase
//! `ERROR_MISSING_OR_INVALID_NONCE`.
//!
//! ## objc2 reconciliation surface (validate on device)
//! Like the other iOS native modules ([`crate::services::media_controls::ios`],
//! [`crate::services::cloud_sync::backend::firebase::appcheck::app_attest`]), this file can only be
//! type-checked with `cargo check --target aarch64-apple-ios` and behaviorally validated on device
//! (or a Simulator signed into an Apple ID). Details that may need adjustment are marked `RECONCILE`:
//! - **objc2 bindings** — exact method names / feature gating across the `objc2-authentication-services`
//!   0.3 surface (init/alloc form, credential accessors, scope-constant nullability).
//! - **presentation anchor** — the key `UIWindow` is fetched by runtime `msg_send!` (objc2-ui-kit is
//!   not a dependency), mirroring the `UIImage` runtime pattern in `media_controls/ios/now_playing.rs`.

use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{
    define_class, msg_send, sel, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly,
};
use objc2_authentication_services::{
    ASAuthorization, ASAuthorizationAppleIDProvider, ASAuthorizationController,
    ASAuthorizationControllerDelegate, ASAuthorizationRequest, ASAuthorizationScopeEmail,
    ASAuthorizationScopeFullName,
};
use objc2_foundation::{NSArray, NSData, NSError, NSString};
use sha2::{Digest, Sha256};
use tauri::AppHandle;
use tokio::sync::oneshot;

use crate::error::{CrateError, Result};

/// Upper bound on the whole native authorization — this leg includes human interaction (the
/// consent sheet + Face ID / Touch ID), so it is generous (matching the desktop browser flow's
/// 300s), unlike the 20s non-interactive App Attest timeout. The Firebase exchange that follows is
/// bounded separately by the command wrapper.
const APPLE_SIGN_IN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// `ASAuthorizationError.canceled`. The user dismissed the sheet — surface a distinct, non-alarming
/// error that the frontend silently swallows. Reuses the desktop loopback flow's exact
/// [`CANCELED`] string so both platforms share one cancel sentinel.
const AS_ERROR_CANCELED: isize = 1001;

/// Cross-platform "the user cancelled sign-in" sentinel (mirrors `oauth_flow::run_loopback_flow`).
/// The mobile store checks for this and shows no error toast.
pub const CANCELED: &str = "sign-in canceled";

/// A successful native Apple authorization, ready for the Firebase `signInWithIdp` exchange.
pub struct AppleCredential {
    /// The Apple identity token (a JWT) — exchanged with Firebase (`providerId = apple.com`).
    pub identity_token: String,
    /// The raw (unhashed) nonce — passed to Firebase, which validates `SHA256(raw)` against the
    /// token's `nonce` claim.
    pub raw_nonce: String,
    /// The user's email (may be an `@privaterelay.appleid.com` relay). Present when granted.
    pub email: Option<String>,
    /// Apple's stable user identifier for this app (diagnostics; Firebase derives the uid itself).
    pub user: Option<String>,
    /// The user's full name — **only present on the FIRST authorization** for a given Apple ID.
    pub full_name: Option<String>,
}

/// Fields extracted from the credential inside the delegate callback. The raw nonce is owned by the
/// async caller and attached afterward (it never crosses into the objc callback).
struct CredentialFields {
    identity_token: String,
    email: Option<String>,
    user: Option<String>,
    full_name: Option<String>,
}

/// Request a native Apple authorization and return the identity token + raw nonce.
///
/// Runs the `ASAuthorizationController` on the main thread (UI + delegate callbacks are
/// main-thread-bound) and bridges the delegate result to async via a `oneshot` + timeout — the same
/// shape as the App Attest bridge, but kicked off on the main thread rather than the blocking pool.
pub async fn request_apple_credential(app: &AppHandle) -> Result<AppleCredential> {
    let raw_nonce = super::oauth_flow::random_b64url(32);
    let hashed_nonce = sha256_hex(&raw_nonce);
    let (tx, rx) = oneshot::channel::<Result<CredentialFields>>();

    // Kick off the native sheet on the main thread. The closure creates all objc2 objects internally
    // (they never cross back out — the retained controller/delegate are stashed in a main-thread
    // thread-local), so it stays `Send + 'static` as `run_on_main_thread` requires.
    app.run_on_main_thread(move || {
        // SAFETY: on the main thread (run_on_main_thread guarantees it), so this marker is valid.
        let mtm =
            MainThreadMarker::new().expect("apple sign-in must be started on the main thread");
        begin_authorization(mtm, &hashed_nonce, tx);
    })
    .map_err(|e| CrateError::CloudSyncAuth(format!("apple sign-in dispatch failed: {e}")))?;

    let outcome = match tokio::time::timeout(APPLE_SIGN_IN_TIMEOUT, rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(CrateError::CloudSyncAuth(
            "apple sign-in: authorization callback dropped".into(),
        )),
        Err(_) => Err(CrateError::CloudSyncAuth("apple sign-in timed out".into())),
    };

    // Release the retained controller + delegate on the main thread now the flow has settled
    // (success, error, or timeout). Cleared here — NOT from inside the delegate callback, which
    // would drop the delegate (`self`) mid-call. Best-effort.
    let _ = app.run_on_main_thread(clear_pending);

    let fields = outcome?;
    Ok(AppleCredential {
        identity_token: fields.identity_token,
        raw_nonce,
        email: fields.email,
        user: fields.user,
        full_name: fields.full_name,
    })
}

/// Build the request + controller and present the sheet. Main thread only.
fn begin_authorization(
    mtm: MainThreadMarker,
    hashed_nonce: &str,
    tx: oneshot::Sender<Result<CredentialFields>>,
) {
    // SAFETY: all AuthenticationServices objects are created + messaged on the main thread (mtm).
    // RECONCILE on device: the init/alloc form + method names against objc2-authentication-services 0.3.
    unsafe {
        let provider = ASAuthorizationAppleIDProvider::new();
        let request = provider.createRequest();

        // Request name + email (both arrive only on the FIRST authorization). RECONCILE: if the
        // scope constants are bound as `Option<&NSString>`, collect the `Some` values instead.
        let scopes: [&NSString; 2] = [ASAuthorizationScopeFullName, ASAuthorizationScopeEmail];
        let scope_array = NSArray::from_slice(&scopes);
        request.setRequestedScopes(Some(&scope_array));

        // SHA256(raw) hex — Apple embeds this in the identity token's `nonce` claim (see module doc).
        let nonce_ns = NSString::from_str(hashed_nonce);
        request.setNonce(Some(&nonce_ns));

        let delegate = SignInDelegate::new(mtm, tx);

        // ASAuthorizationController takes an array of ASAuthorizationRequest (the AppleID request
        // derefs to its ASAuthorizationRequest base).
        let base: &ASAuthorizationRequest = &request;
        let requests = NSArray::from_slice(&[base]);
        let controller = ASAuthorizationController::initWithAuthorizationRequests(
            ASAuthorizationController::alloc(),
            &requests,
        );
        controller.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        // Presentation-context provider set untyped: the typed protocol method returns an
        // `ASPresentationAnchor` the bindings gate to macOS (NSWindow), so we implement the selector
        // manually (see the `impl SignInDelegate` block below) and assign it here by message.
        let _: () = msg_send![&*controller, setPresentationContextProvider: &*delegate];

        // Retain the controller + delegate for the flow's lifetime: ASAuthorizationController keeps
        // only a weak delegate reference and nothing else retains the controller after this returns.
        // A new sign-in overwriting the slot transparently cancels any abandoned one.
        PENDING.with(|slot| {
            *slot.borrow_mut() = Some(PendingSignIn {
                _controller: controller.clone(),
                _delegate: delegate,
            });
        });
        controller.performRequests();
    }
}

// ---------------------------------------------------------------------------
// Delegate — the one place this codebase uses `define_class!` (an ObjC subclass).
// ---------------------------------------------------------------------------

/// Instance state for [`SignInDelegate`]. The class is `MainThreadOnly` and every callback fires on
/// the main thread, so a `RefCell` (take-once) is sufficient — no cross-thread `Mutex` needed.
struct DelegateIvars {
    tx: RefCell<Option<oneshot::Sender<Result<CredentialFields>>>>,
}

define_class!(
    // SAFETY:
    // - `NSObject` has no subclassing requirements.
    // - `MainThreadOnly`: the delegate is created, assigned, and invoked only on the main thread
    //   (ASAuthorizationController delivers all callbacks there), so its `!Send`/`!Sync` ivar is
    //   never touched off-main.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "CrateAppleSignInDelegate"]
    #[ivars = DelegateIvars]
    struct SignInDelegate;

    unsafe impl NSObjectProtocol for SignInDelegate {}

    unsafe impl ASAuthorizationControllerDelegate for SignInDelegate {
        #[unsafe(method(authorizationController:didCompleteWithAuthorization:))]
        fn did_complete(
            &self,
            _controller: &ASAuthorizationController,
            authorization: &ASAuthorization,
        ) {
            // SAFETY: called by the framework on the main thread with a live authorization.
            let result = unsafe { extract_credential(authorization) };
            self.send(result);
        }

        #[unsafe(method(authorizationController:didCompleteWithError:))]
        fn did_error(&self, _controller: &ASAuthorizationController, error: &NSError) {
            self.send(Err(map_error(error)));
        }
    }

    // Presentation-context provider selector, implemented manually (see `begin_authorization`).
    impl SignInDelegate {
        #[unsafe(method(presentationAnchorForAuthorizationController:))]
        fn presentation_anchor(&self, _controller: &ASAuthorizationController) -> *mut AnyObject {
            // SAFETY: runtime traversal to the key UIWindow on the main thread.
            unsafe { key_window() }
        }
    }
);

impl SignInDelegate {
    fn new(mtm: MainThreadMarker, tx: oneshot::Sender<Result<CredentialFields>>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(DelegateIvars {
            tx: RefCell::new(Some(tx)),
        });
        // SAFETY: NSObject's designated initializer.
        unsafe { msg_send![super(this), init] }
    }

    /// Deliver the result exactly once (both delegate callbacks are terminal).
    fn send(&self, result: Result<CredentialFields>) {
        if let Some(tx) = self.ivars().tx.borrow_mut().take() {
            let _ = tx.send(result);
        }
    }
}

// ---------------------------------------------------------------------------
// Controller/delegate lifetime — retained across the async wait, dropped on the main thread.
// ---------------------------------------------------------------------------

struct PendingSignIn {
    _controller: Retained<ASAuthorizationController>,
    _delegate: Retained<SignInDelegate>,
}

thread_local! {
    /// The single in-flight sign-in's retained objc objects (see `begin_authorization`). Cleared by
    /// [`clear_pending`] from the awaiting async task once the flow settles.
    static PENDING: RefCell<Option<PendingSignIn>> = const { RefCell::new(None) };
}

/// Drop the retained controller/delegate on the main thread. Idempotent.
fn clear_pending() {
    PENDING.with(|slot| {
        *slot.borrow_mut() = None;
    });
}

// ---------------------------------------------------------------------------
// Credential extraction + error mapping (main thread, inside the callbacks).
// ---------------------------------------------------------------------------

/// # Safety
/// `authorization` is a live `ASAuthorization` handed to the delegate on the main thread.
/// RECONCILE on device: credential selector names + `NSData`/`NSString` accessors.
unsafe fn extract_credential(authorization: &ASAuthorization) -> Result<CredentialFields> {
    // The credential is an `ASAuthorizationAppleIDCredential`; read its fields via `msg_send` to
    // avoid a protocol-object downcast (and the extra typed feature it would need).
    let credential: Retained<AnyObject> = msg_send![authorization, credential];
    let credential: &AnyObject = &credential;

    let identity_token: *mut NSData = msg_send![credential, identityToken];
    let Some(identity_token) = identity_token.as_ref() else {
        return Err(CrateError::CloudSyncAuth(
            "apple sign-in: missing identity token".into(),
        ));
    };
    let identity_token = String::from_utf8(identity_token.to_vec()).map_err(|_| {
        CrateError::CloudSyncAuth("apple sign-in: identity token was not valid UTF-8".into())
    })?;

    let user = nsstring_to_string(msg_send![credential, user]);
    let email = nsstring_to_string(msg_send![credential, email]);
    let full_name = extract_full_name(msg_send![credential, fullName]);

    Ok(CredentialFields {
        identity_token,
        email,
        user,
        full_name,
    })
}

/// # Safety
/// `ptr` is an autoreleased `NSString*` out of a credential getter (may be null), valid for this call.
unsafe fn nsstring_to_string(ptr: *mut NSString) -> Option<String> {
    ptr.as_ref()
        .map(|s| s.to_string())
        .filter(|s| !s.is_empty())
}

/// # Safety
/// `components` is an autoreleased `NSPersonNameComponents*` (may be null; only set on first sign-in).
/// RECONCILE on device: `givenName`/`familyName` selectors.
unsafe fn extract_full_name(components: *mut AnyObject) -> Option<String> {
    let components = components.as_ref()?;
    let given: *mut NSString = msg_send![components, givenName];
    let family: *mut NSString = msg_send![components, familyName];
    let given = given.as_ref().map(|s| s.to_string()).unwrap_or_default();
    let family = family.as_ref().map(|s| s.to_string()).unwrap_or_default();
    let full = format!("{given} {family}");
    let full = full.trim();
    if full.is_empty() {
        None
    } else {
        Some(full.to_string())
    }
}

/// Map an `ASAuthorizationError` to a `CrateError`, translating user-cancel to the shared sentinel.
fn map_error(error: &NSError) -> CrateError {
    if error.code() == AS_ERROR_CANCELED {
        return CrateError::CloudSyncAuth(CANCELED.into());
    }
    CrateError::CloudSyncAuth(format!(
        "apple sign-in failed: {}",
        error.localizedDescription()
    ))
}

// ---------------------------------------------------------------------------
// Presentation anchor — the app's key UIWindow, via the ObjC runtime (no objc2-ui-kit dependency).
// ---------------------------------------------------------------------------

/// # Safety
/// Called on the main thread. Returns a borrowed (`+0`) `UIWindow*` the framework retains as needed,
/// or null if none is found. RECONCILE on device: the scene walk against the live Tauri WKWebView host.
unsafe fn key_window() -> *mut AnyObject {
    let Some(app_class) = AnyClass::get(c"UIApplication") else {
        return std::ptr::null_mut();
    };
    let app: *mut AnyObject = msg_send![app_class, sharedApplication];
    if app.is_null() {
        return std::ptr::null_mut();
    }

    // Preferred (iOS 13+): connectedScenes → foreground-active UIWindowScene → its key window.
    // `UISceneActivationStateForegroundActive == 0`.
    let scenes: *mut AnyObject = msg_send![app, connectedScenes];
    if !scenes.is_null() {
        let all: *mut AnyObject = msg_send![scenes, allObjects];
        if !all.is_null() {
            let count: usize = msg_send![all, count];
            for i in 0..count {
                let scene: *mut AnyObject = msg_send![all, objectAtIndex: i];
                if scene.is_null() {
                    continue;
                }
                let state: isize = msg_send![scene, activationState];
                if state != 0 {
                    continue;
                }
                // Only a UIWindowScene responds to -windows.
                let has_windows: bool = msg_send![scene, respondsToSelector: sel!(windows)];
                if !has_windows {
                    continue;
                }
                let windows: *mut AnyObject = msg_send![scene, windows];
                if windows.is_null() {
                    continue;
                }
                let wcount: usize = msg_send![windows, count];
                let mut first: *mut AnyObject = std::ptr::null_mut();
                for j in 0..wcount {
                    let win: *mut AnyObject = msg_send![windows, objectAtIndex: j];
                    if win.is_null() {
                        continue;
                    }
                    if first.is_null() {
                        first = win;
                    }
                    let is_key: bool = msg_send![win, isKeyWindow];
                    if is_key {
                        return win;
                    }
                }
                if !first.is_null() {
                    return first;
                }
            }
        }
    }

    // Fallback: the deprecated single-window accessor (Crate is a single-window WKWebView app, so a
    // window always exists when the user taps sign-in).
    msg_send![app, keyWindow]
}

/// SHA-256 of `input`, lowercase hex. Apple/Firebase samples set the request `nonce` to this hex
/// digest of the raw nonce (not base64) — see the module doc.
fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

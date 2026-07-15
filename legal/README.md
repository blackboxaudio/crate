# Legal & Store-Compliance Docs

Source-of-truth legal documents for releasing the Crate mobile app on the Apple App Store
and Google Play. Publisher: **Black Box Audio, LLC** (United States). Privacy contact:
**matthew@bbx-audio.com**.

> These documents were drafted from an audit of the app's actual data behavior. They are a
> solid, accurate starting point — not legal advice. Because the app has user accounts and
> cloud sync, have a lawyer review the final versions (especially the GDPR/CCPA sections)
> before you publish.

## Files

| File | What it is | Where it goes |
|---|---|---|
| `privacy-policy.md` | Privacy policy — editable source of truth | — |
| `privacy-policy.html` | Same content, self-contained styled page | Host at `https://crate.bbx-audio.com/privacy` |
| `account-deletion.md` | Account-deletion instructions — editable source | — |
| `account-deletion.html` | Same content, self-contained styled page | Host at `https://crate.bbx-audio.com/account-deletion` |
| `store-data-safety.md` | Exact answers for Apple App Privacy + Google Data safety forms | Internal reference (do not host) |

The `.html` pages are fully self-contained (no external fonts, scripts, or images) and use
the Crate brand palette, so a privacy page makes zero third-party requests of its own. Edit
the `.md` source first, then mirror changes into the `.html`.

## Hosting

The URLs above assume your product site (`crate.bbx-audio.com`) serves these paths. If you
host elsewhere, update the URLs referenced inside `privacy-policy.*` (the account-deletion
link) and `store-data-safety.md`, and point the store consoles at wherever you host them.

## Release checklist

### Documents (this folder)
- [ ] Host the privacy policy at a public HTTPS URL.
- [ ] Host the account-deletion page at a public HTTPS URL.
- [ ] Paste the privacy-policy URL into **App Store Connect** (App Privacy → Privacy Policy URL)
      and **Google Play Console** (App content → Privacy policy).
- [ ] Fill in **Apple App Privacy** and **Google Data safety** using `store-data-safety.md`.
- [ ] Add the account-deletion URL in **Play Console → App content → Data deletion**.

### Engineering / store requirements still needed for approval
- [ ] **In-app account deletion.** Apple (Guideline 5.1.1(v)) and Google both require an
      in-app path to delete the account when the app supports account creation. The
      account-deletion page describes **Settings → Account → Delete Account**; that flow must
      actually exist and delete the Firebase Auth user + all `users/{uid}` Firestore/Storage
      data before you submit, or Apple will reject the build.
- [ ] **Sign in with Apple.** The iOS app offers Google sign-in as its only third-party
      login. Apple Guideline 4.8 generally requires also offering an equivalent
      privacy-focused option (Sign in with Apple) unless you qualify for an exception. Resolve
      before iOS submission.
- [ ] **Reconcile public docs.** The FAQ (`docs/src/content/docs/faq/index.md`, "Is there
      cloud sync?") still says "No… No internet required," which contradicts the shipped
      Google/Firebase cloud sync and this policy. Update it so your public messaging matches.

### Optional
- [ ] Terms of Service / EULA — not required to launch (Apple's standard EULA and Google's
      default apply otherwise). Ask and this can be drafted next.
- [ ] Add a postal mailing address to the privacy policy contact block — optional, but
      strengthens GDPR Art. 13 / CCPA compliance.

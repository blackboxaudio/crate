# Legal & Store-Compliance Docs

Source-of-truth legal documents for releasing the Crate mobile app on the Apple App Store
and Google Play. Publisher: **Black Box Audio, LLC** (United States). Privacy contact:
**support@bbx-audio.com**.

> These documents were drafted from an audit of the app's actual data behavior. They are a
> solid, accurate starting point — not legal advice. Because the app has user accounts and
> cloud sync, have a lawyer review the final versions (especially the GDPR/CCPA sections)
> before you publish.

## Files

| File | What it is | Where it lives / goes |
|---|---|---|
| `legal/privacy-policy.md` | Privacy policy — editable source of truth | — |
| `legal/account-deletion.md` | Account-deletion instructions — editable source | — |
| `legal/store-data-safety.md` | Exact answers for Apple App Privacy + Google Data safety forms | Internal reference (do not host) |
| `web/privacy/index.html` | Deployable privacy page (self-contained) | Live at `https://crate.bbx-audio.com/privacy/index.html` |
| `web/account-deletion/index.html` | Deployable account-deletion page (self-contained) | Live at `https://crate.bbx-audio.com/account-deletion/index.html` |

The deployable `.html` pages live under `web/` (next to the marketing `index.html`) so the
site deploy publishes them; the editable Markdown sources and the data-safety cheat sheet
stay in `legal/`. The pages are fully self-contained (no external fonts, scripts, or images)
and use the Crate brand palette. Edit the `.md` source first, then mirror changes into the
matching `web/**/index.html`.

## Hosting & deployment

The pages deploy to the `crate-web` Google Cloud Storage bucket (which serves
`crate.bbx-audio.com` via Cloudflare) in the **Deploy landing page** step of
`.github/workflows/cd.release.yml`, so they refresh automatically on every release. They
were also uploaded once manually on 2026-07-15 and are **already live**:

- Privacy policy → `https://crate.bbx-audio.com/privacy/index.html`
- Account deletion → `https://crate.bbx-audio.com/account-deletion/index.html`

**Use the `/index.html` URLs.** The domain (Cloudflare → GCS) maps URL paths literally to
bucket object keys, with no directory-index resolution except at the root, so the
extensionless `https://crate.bbx-audio.com/privacy` returns 404. For clean URLs, add a
Cloudflare rewrite rule (`/privacy` → `/privacy/index.html`, `/account-deletion` →
`/account-deletion/index.html`); until then the `/index.html` URLs are canonical.

## Release checklist

### Documents (this folder) — hosting done
- [x] Pages hosted at public HTTPS URLs (deployed to `gs://crate-web`; auto-deploys on release).
- [ ] Paste `https://crate.bbx-audio.com/privacy/index.html` into **App Store Connect**
      (App Privacy → Privacy Policy URL) and **Google Play Console** (App content → Privacy policy).
- [ ] Fill in **Apple App Privacy** and **Google Data safety** using `store-data-safety.md`.
- [ ] Add `https://crate.bbx-audio.com/account-deletion/index.html` in
      **Play Console → App content → Data deletion**.

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

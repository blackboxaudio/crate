# Store Data-Disclosure Cheat Sheet

Reference answers for **Apple App Privacy** (App Store Connect) and **Google Play Data
safety**. These are prepared for the **mobile app**, whose cloud sync is limited to
discovery/organization data (the mobile app does not sync local-library file paths/hashes;
the desktop app does — not relevant to the store forms).

**Guiding facts (from the codebase):**

- All data collection happens **only if the user opts into cloud sync and signs in with
  Google**. With sync off, nothing is collected. Both stores still require you to declare
  data as "collected"; mark it **optional** where the form allows.
- **No advertising, no analytics, no telemetry, no crash reporting** leaves the device.
- **No tracking** across apps/websites → nothing goes in Apple's "Data Used to Track You,"
  and no App Tracking Transparency prompt is needed.
- Cloud data lives with **Google Firebase**, acting as our processor → in Google Play terms
  this is **collected, not "shared."**
- Data in transit is **encrypted (HTTPS/TLS)**. Users can **request deletion**
  (`https://crate.bbx-audio.com/account-deletion`).

---

## Apple — App Privacy (App Store Connect)

**"Do you or your third-party partners collect data from this app?" → Yes.**

All collected data is **Data Linked to You**. **Nothing** is Data Used to Track You.
Purpose for every item: **App Functionality**. (Do not check Analytics, Advertising,
Product Personalization, or Developer's Advertising/Marketing.)

| Apple data type | Collected? | Linked to identity? | Used for tracking? | Purpose |
|---|---|---|---|---|
| Contact Info → **Email Address** | Yes | Yes | No | App Functionality |
| Contact Info → **Name** | Yes | Yes | No | App Functionality |
| User Content → **Other User Content** (saved releases, playlists, tags, notes, follows) | Yes | Yes | No | App Functionality |
| Identifiers → **User ID** (Firebase account ID) | Yes | Yes | No | App Functionality |
| Identifiers → **Device ID** (app-generated per-device ID for device management) | Yes | Yes | No | App Functionality |

**Everything else → Not Collected**, including: Location, Financial Info, Health &
Fitness, Contacts, Browsing/Search History, Purchases, **Usage Data**, **Diagnostics**
(local only, never transmitted), Sensitive Info, Audio Data, Photos/Videos.

- Profile photo: this is a **link** to the user's Google profile picture, not an image we
  read from the photo library. It's simplest to leave "Photos or Videos" **Not Collected**;
  if a reviewer asks, declare it under **Other Data → Other Data Types**, Linked, App
  Functionality.

---

## Google Play — Data safety

**Does your app collect or share any required or optional user data? → Yes (collect).**

For every item below: **Shared = No** (Firebase is our processor, not a third party we
share with), **Optional = Yes** (only collected when the user enables cloud sync),
**Processed ephemerally = No**.

| Google Play category → type | Collected? | Shared? | Optional? | Purposes |
|---|---|---|---|---|
| Personal info → **Name** | Yes | No | Yes | App functionality, Account management |
| Personal info → **Email address** | Yes | No | Yes | App functionality, Account management |
| Personal info → **User IDs** (Firebase account ID) | Yes | No | Yes | App functionality, Account management |
| App activity → **Other user-generated content** (saved releases, playlists, tags, notes, follows) | Yes | No | Yes | App functionality |
| Device or other IDs → **Device or other IDs** (app-generated per-device ID) | Yes | No | Yes | App functionality |

**Everything else → Not Collected**, including: Location, Financial info, Health &
fitness, Messages, Photos and videos, **Audio files** (previews stream from third parties
and are never uploaded), Files and docs, Calendar, Contacts, **Web browsing**,
**App info and performance → Crash logs / Diagnostics** (local only, never transmitted).

### Security section (Google Play)

- **Is all of the user data collected by your app encrypted in transit?** → **Yes.**
- **Do you provide a way for users to request that their data be deleted?** → **Yes.**
  Deletion URL: `https://crate.bbx-audio.com/account-deletion`
- **Is data collection required to use the app, or can users choose?** → **Users can
  choose** (cloud sync is optional).
- **Has your app been independently validated against a global security standard?** → No
  (unless you complete such a review).

---

## One-line summary for your listing/notes

> Crate is local-first. It collects no data unless you turn on optional cloud sync and sign
> in with Google, in which case it stores your account profile and the library/discovery
> data you choose to sync, solely to sync it across your devices. No ads, no analytics, no
> tracking, and your audio files never leave your device.

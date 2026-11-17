# Gmail IMAP auth from a CLI

Research for [#2](https://github.com/kryptamine/postal/issues/2): can `postal login`
authorise Gmail IMAP through the browser comfortably, and what is the fallback?

Sources were read on 2026-11-17. Every claim links to the page that owns it.

## 1. What Gmail accepts over IMAP

- The account password alone no longer works. Google stopped supporting
  "less secure apps" (username + password over IMAP, SMTP, POP, CalDAV) for
  Workspace accounts on May 1, 2025, and points clients to OAuth
  ([Workspace Updates](https://workspaceupdates.googleblog.com/2023/09/winding-down-google-sync-and-less-secure-apps-support.html),
  [Transition to OAuth](https://support.google.com/a/answer/14114704)).
  Himalaya's docs note the same for personal Gmail: "Gmail rejects the account
  password over SASL PLAIN"
  ([himalaya README](https://github.com/pimalaya/himalaya/blob/master/README.md)).
- Two working credentials remain: an **app password** over `LOGIN`/`PLAIN`, or an
  **OAuth 2.0 access token** over SASL `XOAUTH2`.

## 2. OAuth 2.0 installed-app flow

### Protocol

- `XOAUTH2` sends base64 of `user=<email>\x01auth=Bearer <token>\x01\x01`. On
  failure the server sends a base64 JSON challenge and the client must answer
  with an empty response. `SASL-IR` lets the initial response go on the
  `AUTHENTICATE` line
  ([Google XOAUTH2](https://developers.google.com/workspace/gmail/imap/xoauth2-protocol)).
- The only scope for IMAP/SMTP/POP is `https://mail.google.com/` (full mail
  access). `gmail.imap_admin` exists but only for Workspace domain-wide delegation
  (same page).

### Desktop flow

From [OAuth 2.0 for iOS & Desktop Apps](https://developers.google.com/identity/protocols/oauth2/native-app):

- Recommended redirect on macOS/Linux: **loopback IP**. Start an HTTP listener on
  `127.0.0.1` with a random free port. `localhost` works but "may cause issues with
  client firewalls".
- **PKCE is required** ("Always use PKCE"), method `S256`.
- The manual copy/paste (OOB) flow "is no longer supported", so the browser must
  run on the same machine. Over SSH the user has to forward the port.
- "Desktop apps are public clients and cannot keep secrets confidential." Google
  still issues a client secret for installed apps and says to embed it: "the
  client secret is obviously not treated as a secret"
  ([OAuth 2.0 overview](https://developers.google.com/identity/protocols/oauth2)).
- The **device flow** is no alternative: its allowed scopes are only
  `openid`/`email`/`profile`, two Drive scopes and two YouTube scopes. No Gmail
  scope
  ([Limited-input devices](https://developers.google.com/identity/protocols/oauth2/limited-input-device)).

### Token lifetime

From the [OAuth 2.0 overview, refresh token expiration](https://developers.google.com/identity/protocols/oauth2#expiration):

- Access tokens last about an hour; installed apps always get a refresh token.
- A refresh token stops working when the user revokes access, it goes **unused for
  six months**, the user **changes password and the token has Gmail scopes**, or
  time-based access expires.
- **Testing** publishing status with external users: refresh tokens **expire in
  7 days** (unless the only scopes are name/email/profile). Testing is also
  capped at 100 listed test users
  ([Publishing status](https://support.google.com/cloud/answer/15549945)).
- At most **100 refresh tokens per Google Account per client ID**. The oldest is
  invalidated silently past that.

### Verification and the unverified-app screen

- `mail.google.com` is a **restricted** scope. A public app using it must pass
  brand verification, a demo video, scope justification and an **annual security
  assessment** by a Google-empanelled assessor
  ([Restricted scope verification](https://support.google.com/cloud/answer/13464321)).
- Until verified, the consent flow shows the **unverified app** screen and the
  client is capped at **100 new users in total**
  ([Unverified apps](https://support.google.com/cloud/answer/7454865)).
- Verification is not needed for a **personal use app (fewer than 100 users)**:
  users click through the warning
  ([When verification is not needed](https://support.google.com/cloud/answer/13464323)).
  An "In production" personal client therefore avoids the 7-day Testing expiry at
  the price of the warning screen.

### Embedded client ID vs bring-your-own

| | Embedded Postal client | Bring-your-own (BYO) client |
|---|---|---|
| User effort | Click "Allow" in the browser | Create a Cloud project, enable Gmail API, consent screen, Desktop client, paste ID + secret (~10 console steps) |
| Secret in binary | Fine per Google (public client + PKCE) | n/a |
| Restricted scope | Needs verification + annual security assessment before >100 users | Personal use, no verification |
| Warning screen | Until verified | Always (clicked through once) |
| Refresh token | 6-month idle / password change | Same; **7 days if left in Testing** |
| Blast radius | One quota and one revocation for all users | Per user |

The embedded client is the only path where browser auth is truly comfortable,
and it is gated on a yearly third-party assessment that an open-source hobby
project cannot plan on. The 100-user cap makes an unverified embedded client
unfit for anything public.

## 3. App passwords

From [Sign in with app passwords](https://support.google.com/accounts/answer/185833):

- Require **2-Step Verification**.
- Unavailable when 2SV is set up with security keys only, on **work or school**
  accounts (admin-dependent), and under **Advanced Protection**.
- **Revoked when the Google password changes** (same as OAuth Gmail tokens).
- Google discourages them in favour of "Sign in with Google".
- No other expiry: no refresh logic, no 7-day trap, no warning screen. One
  16-character string the user pastes once, used with `LOGIN`/`PLAIN`.

## 4. How other clients do it

- **himalaya**: app password via `imap.sasl.plain.password.command`
  (e.g. `pass show gmail`); for OAuth it "runs no OAuth flow of its own" and reads
  a token from a helper such as [ortie](https://github.com/pimalaya/ortie) via
  `token.command`
  ([README](https://github.com/pimalaya/himalaya/blob/master/README.md),
  [config.sample.toml](https://github.com/pimalaya/himalaya/blob/master/config.sample.toml)).
- **aerc**: `imaps+xoauth2://` or `imaps+oauthbearer://` with `token_endpoint` and
  `client_id` query params; the stored password is treated as a refresh token.
  No built-in browser flow, no shipped client ID; `source-cred-cmd` for helpers
  ([aerc-imap(5)](https://git.sr.ht/~rjarry/aerc/tree/master/item/doc/aerc-imap.5.scd)).
- **neomutt**: `contrib/oauth2/mutt_oauth2.py`. The user registers their own
  Desktop client, adds themself as a test user, then runs `authcode`,
  `localhostauthcode` or `devicecode`; tokens are kept in a gpg-encrypted file
  ([README](https://github.com/neomutt/neomutt/blob/main/contrib/oauth2/README.md)).
- **mbsync**: `AuthMechs` picks from installed SASL modules (XOAUTH2 needs a
  Cyrus SASL plugin) and `PassCmd` supplies the secret; its docs do not mention
  OAuth at all ([mbsync(1)](https://isync.sourceforge.io/mbsync.html)).

Takeaway: none of them ships a verified Google client. Terminal clients push
OAuth to BYO client registration or an external token helper; app passwords are
the common, documented easy path.

## 5. Rust building blocks

- [`oauth2`](https://docs.rs/oauth2/latest/oauth2/) 5.0: authorization code grant
  with `PkceCodeChallenge`, `RefreshTokenRequest`.
- [`async-imap`](https://docs.rs/async-imap/latest/async_imap/struct.Client.html)
  0.12: `Client::authenticate("XOAUTH2", authenticator)` with an `Authenticator`
  returning `user=…\x01auth=Bearer …\x01\x01`; its docs include that example.
  Plain `login()` covers app passwords.
- The loopback listener is a one-request HTTP server on `127.0.0.1:0`; no web
  framework needed.

## Recommendation

**Onboarding path: app password, browser-assisted.** `postal login` asks for the
address, opens `https://myaccount.google.com/apppasswords` in the browser, and
reads the pasted app password into the OS keychain. IMAP uses `LOGIN`. It is one
step, never expires on a timer, shows no warning screen and needs no refresh code.
Onboarding re-runs when the server rejects it (password change, revocation).

**Fallback: OAuth 2.0 loopback + PKCE + `XOAUTH2` with a bring-your-own client
ID.** For accounts that cannot create app passwords (Advanced Protection,
security-key-only 2SV, Workspace that disallows them). `postal login --oauth`
takes the user's Desktop client ID and secret, runs the loopback flow, stores the
refresh token in the keychain and refreshes access tokens on connect. Onboarding
must tell the user to set the consent screen to **In production** (personal use)
rather than Testing, or the token dies after 7 days.

**Not now: an embedded Postal client ID.** It is the only truly comfortable
browser flow, but `mail.google.com` is restricted: public use needs verification
plus an annual security assessment, and unverified it caps at 100 users behind a
warning screen. Keep the credential an enum (app password | OAuth client +
refresh token) so a verified embedded client can replace BYO later without
touching IMAP code.

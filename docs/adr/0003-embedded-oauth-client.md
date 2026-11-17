# Onboarding signs in through Postal's own unverified OAuth client

Onboarding should end in one browser click, so `postal login` runs OAuth
(loopback redirect + PKCE, `AUTHENTICATE XOAUTH2`) with a Google client ID
that ships in the source, not in a build-time variable. Google treats an
installed app's client secret as public, and `cargo install` then works with no
setup. The refresh token, not the access token, lives in the secret store.

The Gmail scope `https://mail.google.com/` is restricted. Unverified, the
client shows an "app not verified" screen and is capped at 100 users for its
lifetime. That is acceptable for the MVP; verification is needed before a
public release.

An app password (`postal login --app-password`) stays as the fallback: for
Workspace accounts whose admin blocks third-party apps, and once the user cap
is reached.

## Considered options

- **App password only.** No Google project to run, but the user must enable
  2-Step Verification, find the app-passwords page and paste a code.
- **Bring-your-own OAuth client.** About ten Cloud Console steps per user, and
  tokens expire in seven days while the client stays in Testing. Harder than an
  app password, so not offered.
- **Verified client now.** Needs a domain, privacy policy and a paid yearly
  security assessment; weeks of work before the first user.

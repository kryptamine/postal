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

OAuth is the only way to sign in to Google. An app password was the fallback
for Workspace accounts whose admin blocks third-party apps and for the user
cap; it was removed to keep one sign-in path, so those accounts cannot use
Postal.

## Amendment: a password only where there is no OAuth

A password is accepted only for a Provider with no OAuth. iCloud offers a
third-party IMAP client none, so an iCloud Account signs in with an
app-specific password, typed into a masked field and checked by `LOGIN` on the
IMAP server before anything is written. It lives in the secret store where a
refresh token would, and SMTP signs in with it too. A Provider that has OAuth,
as Google does, is never offered a password.

## Amendment: any IMAP/SMTP server by password

The generic Provider, `other`, signs in by password to any server: Onboarding
asks for the address, then the IMAP and SMTP servers, guessed from the domain
(`imap.<domain>` on 993, `smtp.<domain>` on 465), then the password, checked by
`LOGIN` before anything is written. The servers are written to the Account's
own `[[account]]` table, never the top level, whose tables every Account
shares. SMTP may take a username and a password of its own; that password is
kept in the secret store under `smtp:<address>`; left blank, it is the IMAP one,
or the one stored while SMTP keeps a username. IMAP takes implicit TLS only:
STARTTLS on IMAP waits for a server that needs it.

## Considered options

- **App password only.** No Google project to run, but the user must enable
  2-Step Verification, find the app-passwords page and paste a code.
- **Bring-your-own OAuth client.** About ten Cloud Console steps per user, and
  tokens expire in seven days while the client stays in Testing. Harder than an
  app password, so not offered.
- **Verified client now.** Needs a domain, privacy policy and a paid yearly
  security assessment; weeks of work before the first user.

# The site lives at postalmail.dev

Google's verification of the OAuth client (ADR 0003) needs a homepage and
a privacy policy on a domain the project owns and has verified in Search
Console, and a public release needs a page that says what Postal is
(issue #107). Both are the site in `landing/`, served at
**`https://postalmail.dev`**.

The short names were taken: `postal.dev`, `postal.app`, `postal.email`,
`postal.sh`. `postalmail.dev` also says what Postal is, which sets it apart
from Postal the mail server (`postalserver/postal`), a different project
under the same name. `.dev` is preloaded as HTTPS-only, which the host
serves anyway.

- **Homepage:** `https://postalmail.dev/`
- **Privacy policy:** `https://postalmail.dev/privacy/`
- **Terms:** `https://postalmail.dev/terms/`
- **Contact:** `privacy@postalmail.dev`, named on both policies and on the
  OAuth consent screen.

The domain, the contact address and the date the policies take effect are
written once, in `landing/src/site.ts`; every page reads them from there.

The site is static. `.github/workflows/landing.yml` builds `landing/` on
a push to `main` that touches it and deploys it to GitHub Pages; the
custom domain is set in the repository's Pages settings, not by a
`CNAME` file, which a deployment from Actions ignores.

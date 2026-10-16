# Inspector promotions

The inspector has a small promotion card beneath its scrollable content. With no active campaigns it produces no card, border or empty space. Campaigns are labeled **From Matily**, separate from scan results. Content uses plain text and bundled icons, with no remote HTML, scripts, images, analytics, impression events or archive data.

Multiple campaigns rotate every 10–120 seconds. Hover, keyboard focus, a hidden window/tab, reduced-motion preference and the pause button stop automatic rotation. Dots switch campaigns manually. Close dismisses that campaign ID across restarts; use a new ID for a new campaign.

## Current build

Hidden and network-inactive: no signing key or live feed has been configured. A public key must be embedded in a build **before** that build can receive remote campaigns. After that initial setup, changing the signed JSON enables/disables campaigns without another application update. No production signing key was generated and no feed was published during implementation.

The fixed endpoint is:
`https://raw.githubusercontent.com/MatilyLabs/MaxRAR/main/promotions/feed.json`

If the repository uses a different branch, change the endpoint before shipping the configured build. The Rust client checks at startup and hourly while running, with a persistent 24-hour network limit. It caches only signed valid content. Failed checks keep a still-valid cached feed; expired, malformed, unsigned or unconfigured content stays hidden. A valid signed disabled feed replaces the cached feed. Shutdowns and clock changes do not bypass signature validation. A feed expiry limits how long a cached campaign can remain visible offline.

## One-time release configuration

Use a dedicated Minisign promotion key, separate from updater signing. Keep the private key outside the repository (the ignored `signing-keys/` directory can hold it locally). Embed only the public key's base64 line through `MAXRAR_PROMOTIONS_PUBLIC_KEY` when building MaxRAR. Do not rotate that public key without releasing an app build that trusts the replacement.

Links currently allow HTTPS `www.matily.org` URLs and GitHub URLs under `/MatilyLabs`, with no credentials, query parameters or fragment. Additional product domains require an explicit native allowlist change and application update. The UI sends only the campaign ID when clicked; native code re-verifies the cached campaign, schedule and destination before opening the system browser.

## Payload schema

`promotions/payload.example.json` is an inactive template, **not a deployable feed**. Set `validUntil` to a future Unix timestamp in seconds, at most 31 days ahead; even disabled feeds need a valid expiry. `startsAt` and `endsAt` are optional/null Unix timestamps in seconds. At most eight unique campaign IDs; title ≤64 characters, description ≤160, button ≤28. Allowed icon values: `app`, `scan`, `archive`.

Example campaign (place inside `campaigns`):

```json
{
  "id": "product-launch-1",
  "title": "More from Matily",
  "description": "Discover our latest tools.",
  "button": "Explore products",
  "url": "https://www.matily.org",
  "icon": "app",
  "startsAt": null,
  "endsAt": null
}
```

Sign the exact payload bytes with Minisign, then package the detached signature:

```sh
minisign -S -s signing-keys/promotions.key -m promotions/payload.json
python3 scripts/package-promotion.py promotions/payload.json promotions/payload.json.minisig promotions/feed.json
```

The envelope contains `payload` (the exact JSON string) and `signature` (the complete Minisign signature text). JSON reformatting after signing invalidates the signature. Publish `promotions/feed.json` only when ready; the repository currently remains local/unpublished. For a kill switch, sign a fresh payload with `enabled: false` and a new expiry. Connected clients receive it on their next daily check; cached campaigns can remain visible until that check or expiry.

## Verification

Frontend tests in `verification/promotions.test.mjs` exercise campaign visibility, schedule and dismissal. Native tests check expiry, invalid destinations, duplicate IDs and signature rejection. Live signed feed delivery and installed cross-platform behavior still need release testing once keys/feed exist.

Signature test fixtures under `verification/fixtures/promotions/` contain public data only, signed using an ephemeral discarded test key. Never use that public key for production.

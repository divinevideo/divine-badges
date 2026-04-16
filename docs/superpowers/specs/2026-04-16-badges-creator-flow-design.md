# Divine Badges Creator Flow Design

**Date:** 2026-04-16

**Goal**

Add the public creator-side badge flow to `badges.divine.video` so any logged-in Nostr user can define a badge, upload its media through the Divine Blossom host, and then award that badge from its public detail page using the `badges.page` interaction model.

## Scope

This design covers two connected surfaces:

1. `/new` as the badge-definition studio for `kind:30009`
2. `/b/:coord` as the owner-only awarding console for `kind:8`

It does not change the landing page, the scheduled Divine issuer Worker flow, or the existing holder-side `Accepted | Awarded | Created` model.

## Product Direction

The app remains Divine-first in presentation, but badge creation is not restricted to the Divine official issuer. Any logged-in user can create badge definitions and award only badges they authored.

The protocol boundary stays explicit:

- badge definition happens first as `kind:30009`
- awarding happens second as `kind:8`

That matches the Nostr badge model cleanly and avoids a large combined wizard with fragile partial-failure states.

## User Experience

### `/new`

`/new` is a creator studio, not a raw protocol form.

The page has three regions:

1. A live preview of the badge as it will appear publicly
2. A focused badge-definition form
3. A publish area with signer state and action feedback

The form fields are:

- badge name
- slug / identifier (`d`)
- description
- primary image upload
- optional custom thumbnail upload

Default media behavior:

- uploading a primary image fills both `image` and `thumb`
- users may optionally upload a second asset to override the thumb

Slug behavior:

- the slug is derived from the badge name by default
- once the user edits the slug manually, it stops auto-syncing from the name

After a successful publish, the user is redirected to `/b/:coord?award=1`.

### `/b/:coord`

The badge page remains public-first, but the owner gets a stronger workspace for issuing awards.

Everyone sees:

- badge hero
- issuer identity
- description
- recent awardees
- technical details only where secondary

If the signed-in pubkey authored the badge, the page also shows an `Award this badge` panel near the top. If the route includes `?award=1`, that panel opens automatically.

Awarding mirrors `badges.page`:

- one NIP-05 input with explicit resolve affordance
- one bulk recipient textarea
- support for `npub`, hex pubkeys, and resolved NIP-05 identities
- dedupe before signing
- one `kind:8` event with one `a` tag and one `p` tag per recipient

Non-owners do not see any awarding controls.

## Media Handling

Badge media is uploaded to the Divine Blossom host on `media.divine.video`.

The app should handle actual upload and store the returned media URLs, rather than asking users to paste asset URLs manually.

Publish ordering:

1. upload primary image
2. optionally upload thumb override
3. build `kind:30009`
4. sign and publish

If upload fails, publishing must not proceed.

If publishing fails after upload succeeds, the uploaded URLs remain in client state so the user can retry without re-uploading.

## Data And Protocol Model

### Badge Definition

The definition event is authored by the logged-in pubkey and contains:

- `["d", slug]`
- `["name", badgeName]`
- `["description", description]`
- `["image", imageUrl]`
- `["thumb", thumbUrl]`

The canonical coordinate remains:

`30009:<author_pubkey>:<d>`

### Awarding

The award event is owner-authored and contains:

- one `["a", badgeCoordinate]`
- one `["p", recipientPubkey]` per deduped recipient

NIP-05 input is a convenience layer only. Final published recipients must always be normalized pubkeys.

## Error Handling

### `/new`

- logged-out users can load the page, but publish actions collapse to a clear `Log in to sign` state
- upload failures surface inline and block publish
- publish failures surface inline and preserve already-uploaded media URLs and current form values

### `/b/:coord`

- invalid or unresolvable recipients are shown in a visible invalid list and excluded from publish
- duplicate recipients are collapsed before signing
- if the signer is not the badge author, the award panel does not render
- if award publish fails, the resolved-recipient list remains visible for retry

## Testing Strategy

The next implementation should be test-first for the new pure logic:

- slug generation and manual override behavior
- owner-only award visibility logic
- badge definition builder using uploaded image and thumb URLs
- bulk recipient parsing and dedupe
- award builder for mixed recipient input

Manual browser verification should then confirm:

- creating with one image
- creating with image plus custom thumb
- redirect to owner award mode after publish
- bulk award publish flow
- non-owner badge page hides award controls

## Notes

This is intentionally not a framework rewrite. The Rust Worker stays the route shell and static asset server, with browser-side ES modules handling signer state, Blossom upload, and relay interaction.

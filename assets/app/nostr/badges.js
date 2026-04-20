import {
  BADGE_AWARD,
  BADGE_DEFINITION,
  CONTACT_LIST,
  PROFILE_BADGES,
  PROFILE_BADGES_D,
} from "./constants.js?v=2026-04-14-1";
import { canonicalBadgePath } from "./identity.js?v=2026-04-20-1";

export function deriveBadgeSlug(name) {
  return (name || "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

export function parseRecipientInput(value) {
  const parts = (value || "")
    .split(/[\n,]+/)
    .map((part) => part.trim())
    .filter(Boolean);
  return [...new Set(parts)];
}

export function canAwardBadge({ signerPubkey, badgeAuthorPubkey }) {
  return Boolean(signerPubkey) && signerPubkey === badgeAuthorPubkey;
}

export function buildNewBadgePreviewModel({
  name,
  description,
  identifier,
  imageUrl,
  thumbUrl,
}) {
  return {
    name: name || "Untitled badge",
    description: description || "",
    identifier: identifier || "",
    imageUrl: imageUrl || null,
    thumbUrl: thumbUrl || imageUrl || null,
  };
}

export function shouldOpenAwardPanel(search) {
  return new URLSearchParams(search).get("award") === "1";
}

export function findTag(tags, key) {
  return tags.find((tag) => tag[0] === key)?.[1];
}

export function coordinateFromBadgeDefinition(badge) {
  return `${badge.kind}:${badge.pubkey}:${findTag(badge.tags, "d")}`;
}

export function coordinatePathFromBadge(badge) {
  return canonicalBadgePath({
    kind: badge.kind,
    pubkey: badge.pubkey,
    identifier: findTag(badge.tags, "d"),
  });
}

export function buildCreatedBadgeActions({ badge, isOwner }) {
  const base = coordinatePathFromBadge(badge);
  const actions = {
    view: { label: "View", href: base },
    share: { label: "Copy link", href: base },
    edit: null,
    award: null,
  };
  if (isOwner) {
    actions.edit = { label: "Edit", href: `${base}/edit` };
    actions.award = { label: "Award", href: `${base}?award=1` };
  }
  return actions;
}

export function extractProfileBadgePairs(profileEvent) {
  const pairs = [];
  if (!profileEvent?.tags) {
    return pairs;
  }
  let current = null;
  for (const tag of profileEvent.tags) {
    if (tag[0] === "a") {
      if (current) {
        pairs.push(current);
      }
      current = {
        a: tag[1],
        aRelay: tag[2],
      };
    } else if (tag[0] === "e" && current) {
      current.e = tag[1];
      current.eRelay = tag[2];
    }
  }
  if (current) {
    pairs.push(current);
  }
  return pairs;
}

export function buildProfileBadgeTags(pairs) {
  const tags = [["d", PROFILE_BADGES_D]];
  for (const pair of pairs) {
    if (pair.a) {
      tags.push(pair.aRelay ? ["a", pair.a, pair.aRelay] : ["a", pair.a]);
    }
    if (pair.e) {
      tags.push(pair.eRelay ? ["e", pair.e, pair.eRelay] : ["e", pair.e]);
    }
  }
  return tags;
}

export function buildAcceptProfileBadgesEvent({
  pubkey,
  profileEvent,
  badgeCoordinate,
  awardId,
  relayUrl,
  createdAt,
}) {
  if (profileEvent === null || profileEvent === undefined) {
    throw new Error(
      "profileEvent is required to preserve existing profile_badges; fetch the latest kind:30008 first"
    );
  }
  const pairs = extractProfileBadgePairs(profileEvent);
  pairs.push({
    a: badgeCoordinate,
    aRelay: relayUrl,
    e: awardId,
    eRelay: relayUrl,
  });
  return {
    kind: PROFILE_BADGES,
    pubkey,
    content: "",
    tags: buildProfileBadgeTags(pairs),
    created_at: createdAt,
  };
}

export function buildHideProfileBadgesEvent({
  pubkey,
  profileEvent,
  awardId,
  createdAt,
}) {
  if (profileEvent === null || profileEvent === undefined) {
    throw new Error(
      "profileEvent is required to preserve existing profile_badges; fetch the latest kind:30008 first"
    );
  }
  const filteredPairs = extractProfileBadgePairs(profileEvent).filter(
    (pair) => pair.e !== awardId
  );
  return {
    kind: PROFILE_BADGES,
    pubkey,
    content: "",
    tags: buildProfileBadgeTags(filteredPairs),
    created_at: createdAt,
  };
}

export function buildBadgeDefinitionEvent({
  pubkey,
  identifier,
  slug,
  name,
  description,
  imageUrl,
  image,
  thumbUrl,
  thumb,
  createdAt,
}) {
  const resolvedIdentifier = identifier || slug;
  const resolvedImage = imageUrl || image || "";
  const resolvedThumb = thumbUrl || thumb || "";
  return {
    kind: BADGE_DEFINITION,
    pubkey,
    content: "",
    created_at: createdAt,
    tags: [
      ["d", resolvedIdentifier],
      ["name", name],
      ["description", description || ""],
      ["image", resolvedImage],
      ["thumb", resolvedThumb],
    ],
  };
}

export function buildEditedBadgeDefinitionEvent({
  existingEvent,
  pubkey,
  name,
  description,
  imageUrl,
  thumbUrl,
  createdAt,
}) {
  if (!existingEvent) {
    throw new Error("existingEvent is required to edit a badge definition");
  }
  const identifier = findTag(existingEvent.tags || [], "d");
  if (!identifier) {
    throw new Error("existing badge event is missing d tag");
  }
  return {
    kind: BADGE_DEFINITION,
    pubkey,
    content: "",
    created_at: createdAt,
    tags: [
      ["d", identifier],
      ["name", name],
      ["description", description || ""],
      ["image", imageUrl || ""],
      ["thumb", thumbUrl || ""],
    ],
  };
}

export function buildBadgeAwardEvent({ pubkey, badgeCoordinate, recipients, createdAt }) {
  const uniqueRecipients = [...new Set(recipients)];
  return {
    kind: BADGE_AWARD,
    pubkey,
    content: "",
    created_at: createdAt,
    tags: [["a", badgeCoordinate], ...uniqueRecipients.map((recipient) => ["p", recipient])],
  };
}

export function buildAwardedBadgeRecords(awards, badgeDefinitions) {
  const definitionsByCoordinate = new Map(
    badgeDefinitions.map((badge) => [coordinateFromBadgeDefinition(badge), badge])
  );
  return awards
    .map((award) => {
      const coordinate = findTag(award.tags, "a");
      const badge = definitionsByCoordinate.get(coordinate);
      if (!coordinate || !badge) {
        return null;
      }
      return {
        award,
        badge,
        coordinate,
      };
    })
    .filter(Boolean)
    .sort((left, right) => right.award.created_at - left.award.created_at);
}

export function awardIncludesRecipient(award, pubkey) {
  if (!award?.tags || !pubkey) return false;
  return award.tags.some((tag) => tag[0] === "p" && tag[1] === pubkey);
}

export function buildBadgeViewerCollectionState({
  signerPubkey,
  badgeCoordinate,
  awards,
  profileEvent,
}) {
  if (!signerPubkey) return { status: "logged-out" };
  const award = (awards || []).find(
    (candidate) =>
      findTag(candidate.tags || [], "a") === badgeCoordinate &&
      awardIncludesRecipient(candidate, signerPubkey)
  );
  if (!award) return { status: "not-awarded" };
  const pair = extractProfileBadgePairs(profileEvent).find(
    (candidate) => candidate.a === badgeCoordinate && candidate.e === award.id
  );
  if (pair) return { status: "accepted", award, pair };
  return { status: "awarded", award };
}

export function extractAwardeePubkeys(awards) {
  const seen = new Set();
  for (const award of awards || []) {
    for (const tag of award?.tags || []) {
      if (tag[0] === "p" && typeof tag[1] === "string" && tag[1]) {
        seen.add(tag[1]);
      }
    }
  }
  return [...seen];
}

export function buildFollowAwardeesEvent({
  pubkey,
  contactListEvent,
  awardeePubkeys,
  createdAt,
}) {
  if (contactListEvent === null || contactListEvent === undefined) {
    throw new Error(
      "contactListEvent is required to build a follow-awardees event; fetch the latest kind:3 first"
    );
  }
  const existingTags = contactListEvent.tags || [];
  const existingPubkeys = new Set();
  const pTags = [];
  const otherTags = [];
  for (const tag of existingTags) {
    if (tag[0] === "p" && typeof tag[1] === "string" && tag[1]) {
      if (!existingPubkeys.has(tag[1])) {
        existingPubkeys.add(tag[1]);
        pTags.push(tag);
      }
    } else {
      otherTags.push(tag);
    }
  }
  for (const pk of awardeePubkeys || []) {
    if (typeof pk === "string" && pk && !existingPubkeys.has(pk)) {
      existingPubkeys.add(pk);
      pTags.push(["p", pk]);
    }
  }
  return {
    kind: CONTACT_LIST,
    pubkey,
    content: contactListEvent.content || "",
    created_at: createdAt,
    tags: [...pTags, ...otherTags],
  };
}

export function buildAcceptedBadgeRecords(profileEvent, awards, badgeDefinitions) {
  const awardsById = new Map(awards.map((award) => [award.id, award]));
  const definitionsByCoordinate = new Map(
    badgeDefinitions.map((badge) => [coordinateFromBadgeDefinition(badge), badge])
  );
  return extractProfileBadgePairs(profileEvent)
    .map((pair) => {
      const badge = definitionsByCoordinate.get(pair.a);
      const award = awardsById.get(pair.e);
      if (!badge || !award) {
        return null;
      }
      return {
        award,
        badge,
        coordinate: pair.a,
      };
    })
    .filter(Boolean);
}

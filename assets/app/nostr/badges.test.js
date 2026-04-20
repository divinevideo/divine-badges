import test from "node:test";
import assert from "node:assert/strict";

import {
  awardIncludesRecipient,
  buildAcceptedBadgeRecords,
  buildAcceptProfileBadgesEvent,
  buildAwardedBadgeRecords,
  buildBadgeAwardEvent,
  buildBadgeDefinitionEvent,
  buildBadgeViewerCollectionState,
  buildNewBadgePreviewModel,
  canAwardBadge,
  coordinatePathFromBadge,
  deriveBadgeSlug,
  buildHideProfileBadgesEvent,
  parseRecipientInput,
  buildProfileBadgeTags,
  coordinateFromBadgeDefinition,
  extractProfileBadgePairs,
  shouldOpenAwardPanel,
} from "./badges.js";
import { parseNaddr } from "./identity.js";

test("extractProfileBadgePairs preserves ordered a/e pairs", () => {
  const profileEvent = {
    tags: [
      ["d", "profile_badges"],
      ["a", "30009:issuer:day", "wss://relay.example"],
      ["e", "award-1", "wss://relay.example"],
      ["a", "30009:issuer:week"],
      ["e", "award-2"],
    ],
  };

  assert.deepEqual(extractProfileBadgePairs(profileEvent), [
    {
      a: "30009:issuer:day",
      aRelay: "wss://relay.example",
      e: "award-1",
      eRelay: "wss://relay.example",
    },
    {
      a: "30009:issuer:week",
      aRelay: undefined,
      e: "award-2",
      eRelay: undefined,
    },
  ]);
});

test("buildAcceptProfileBadgesEvent appends a new pair", () => {
  const profileEvent = {
    tags: [
      ["d", "profile_badges"],
      ["a", "30009:issuer:day"],
      ["e", "award-1"],
    ],
  };

  const event = buildAcceptProfileBadgesEvent({
    pubkey: "user-pubkey",
    profileEvent,
    badgeCoordinate: "30009:issuer:week",
    awardId: "award-2",
    relayUrl: "wss://relay.divine.video",
    createdAt: 1234,
  });

  assert.equal(event.kind, 30008);
  assert.equal(event.pubkey, "user-pubkey");
  assert.equal(event.created_at, 1234);
  assert.deepEqual(event.tags, [
    ["d", "profile_badges"],
    ["a", "30009:issuer:day"],
    ["e", "award-1"],
    ["a", "30009:issuer:week", "wss://relay.divine.video"],
    ["e", "award-2", "wss://relay.divine.video"],
  ]);
});

test("buildHideProfileBadgesEvent removes only the targeted award pair", () => {
  const profileEvent = {
    tags: [
      ["d", "profile_badges"],
      ["a", "30009:issuer:day"],
      ["e", "award-1"],
      ["a", "30009:issuer:week"],
      ["e", "award-2"],
    ],
  };

  const event = buildHideProfileBadgesEvent({
    pubkey: "user-pubkey",
    profileEvent,
    awardId: "award-1",
    createdAt: 5678,
  });

  assert.equal(event.kind, 30008);
  assert.equal(event.pubkey, "user-pubkey");
  assert.equal(event.created_at, 5678);
  assert.deepEqual(event.tags, [
    ["d", "profile_badges"],
    ["a", "30009:issuer:week"],
    ["e", "award-2"],
  ]);
});

test("buildProfileBadgeTags rebuilds ordered tag lists from pairs", () => {
  assert.deepEqual(
    buildProfileBadgeTags([
      { a: "30009:issuer:day", e: "award-1" },
      {
        a: "30009:issuer:week",
        aRelay: "wss://relay.example",
        e: "award-2",
        eRelay: "wss://relay.example",
      },
    ]),
    [
      ["d", "profile_badges"],
      ["a", "30009:issuer:day"],
      ["e", "award-1"],
      ["a", "30009:issuer:week", "wss://relay.example"],
      ["e", "award-2", "wss://relay.example"],
    ]
  );
});

test("coordinateFromBadgeDefinition builds the canonical a-tag value", () => {
  assert.equal(
    coordinateFromBadgeDefinition({
      kind: 30009,
      pubkey: "issuer",
      tags: [["d", "diviner-of-the-day"]],
    }),
    "30009:issuer:diviner-of-the-day"
  );
});

test("coordinatePathFromBadge builds a canonical /b/ URL", () => {
  const path = coordinatePathFromBadge({
    kind: 30009,
    pubkey: "0".repeat(64),
    tags: [["d", "scene-stealer"], ["name", "Scene Stealer"]],
  });
  assert.ok(path.startsWith("/b/"));
  const naddr = decodeURIComponent(path.slice(3));
  const parsed = parseNaddr(naddr);
  assert.equal(parsed.kind, 30009);
  assert.equal(parsed.pubkey, "0".repeat(64));
  assert.equal(parsed.identifier, "scene-stealer");
});

test("buildAwardedBadgeRecords joins awards to badge definitions", () => {
  const awards = [
    {
      id: "award-1",
      created_at: 20,
      tags: [["a", "30009:issuer:diviner-of-the-day"], ["p", "user"]],
    },
  ];
  const definitions = [
    {
      id: "badge-1",
      kind: 30009,
      pubkey: "issuer",
      created_at: 10,
      tags: [["d", "diviner-of-the-day"], ["name", "Diviner of the Day"]],
    },
  ];

  assert.deepEqual(buildAwardedBadgeRecords(awards, definitions), [
    {
      award: awards[0],
      badge: definitions[0],
      coordinate: "30009:issuer:diviner-of-the-day",
    },
  ]);
});

test("buildAcceptedBadgeRecords keeps the order from profile badges", () => {
  const awards = [
    {
      id: "award-1",
      created_at: 10,
      tags: [["a", "30009:issuer:diviner-of-the-day"], ["p", "user"]],
    },
    {
      id: "award-2",
      created_at: 20,
      tags: [["a", "30009:issuer:diviner-of-the-week"], ["p", "user"]],
    },
  ];
  const definitions = [
    {
      id: "badge-1",
      kind: 30009,
      pubkey: "issuer",
      created_at: 1,
      tags: [["d", "diviner-of-the-day"]],
    },
    {
      id: "badge-2",
      kind: 30009,
      pubkey: "issuer",
      created_at: 2,
      tags: [["d", "diviner-of-the-week"]],
    },
  ];
  const profileEvent = {
    tags: [
      ["d", "profile_badges"],
      ["a", "30009:issuer:diviner-of-the-week"],
      ["e", "award-2"],
      ["a", "30009:issuer:diviner-of-the-day"],
      ["e", "award-1"],
    ],
  };

  assert.deepEqual(buildAcceptedBadgeRecords(profileEvent, awards, definitions), [
    {
      award: awards[1],
      badge: definitions[1],
      coordinate: "30009:issuer:diviner-of-the-week",
    },
    {
      award: awards[0],
      badge: definitions[0],
      coordinate: "30009:issuer:diviner-of-the-day",
    },
  ]);
});

test("buildBadgeAwardEvent deduplicates recipients", () => {
  assert.deepEqual(
    buildBadgeAwardEvent({
      pubkey: "issuer",
      badgeCoordinate: "30009:issuer:diviner-of-the-day",
      recipients: ["alice", "alice", "bob"],
      createdAt: 42,
    }),
    {
      kind: 8,
      pubkey: "issuer",
      content: "",
      created_at: 42,
      tags: [
        ["a", "30009:issuer:diviner-of-the-day"],
        ["p", "alice"],
        ["p", "bob"],
      ],
    }
  );
});

test("deriveBadgeSlug normalizes badge names into canonical identifiers", () => {
  assert.equal(deriveBadgeSlug("Diviner of the Day"), "diviner-of-the-day");
});

test("buildBadgeDefinitionEvent uses explicit image and thumb URLs", () => {
  const event = buildBadgeDefinitionEvent({
    pubkey: "abc123",
    identifier: "diviner-of-the-day",
    name: "Diviner of the Day",
    description: "Awarded daily",
    imageUrl: "https://media.divine.video/image.webp",
    thumbUrl: "https://media.divine.video/thumb.webp",
    createdAt: 123,
  });

  assert.equal(event.kind, 30009);
  assert.equal(event.pubkey, "abc123");
  assert.equal(event.created_at, 123);
  assert.deepEqual(event.tags, [
    ["d", "diviner-of-the-day"],
    ["name", "Diviner of the Day"],
    ["description", "Awarded daily"],
    ["image", "https://media.divine.video/image.webp"],
    ["thumb", "https://media.divine.video/thumb.webp"],
  ]);
});

test("parseRecipientInput splits and deduplicates mixed separators", () => {
  assert.deepEqual(
    parseRecipientInput("npub1alice\nabcdef1234, abcdef1234 , npub1alice"),
    ["npub1alice", "abcdef1234"]
  );
});

test("canAwardBadge allows only the badge author", () => {
  assert.equal(
    canAwardBadge({ signerPubkey: "owner", badgeAuthorPubkey: "owner" }),
    true
  );
  assert.equal(
    canAwardBadge({ signerPubkey: "viewer", badgeAuthorPubkey: "owner" }),
    false
  );
});

test("buildNewBadgePreviewModel falls back to the primary image for thumb", () => {
  assert.deepEqual(
    buildNewBadgePreviewModel({
      name: "Diviner of the Day",
      description: "Awarded daily",
      identifier: "diviner-of-the-day",
      imageUrl: "https://media.divine.video/image.webp",
      thumbUrl: null,
    }),
    {
      name: "Diviner of the Day",
      description: "Awarded daily",
      identifier: "diviner-of-the-day",
      imageUrl: "https://media.divine.video/image.webp",
      thumbUrl: "https://media.divine.video/image.webp",
    }
  );
});

test("shouldOpenAwardPanel reads the route query flag", () => {
  assert.equal(shouldOpenAwardPanel("?award=1"), true);
  assert.equal(shouldOpenAwardPanel("?award=0"), false);
  assert.equal(shouldOpenAwardPanel(""), false);
});

test("awardIncludesRecipient matches a p-tag for the given pubkey", () => {
  const award = {
    id: "award-1",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "alice"],
      ["p", "bob"],
    ],
  };
  assert.equal(awardIncludesRecipient(award, "alice"), true);
  assert.equal(awardIncludesRecipient(award, "bob"), true);
});

test("awardIncludesRecipient returns false when the pubkey is absent", () => {
  const award = {
    id: "award-1",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "alice"],
    ],
  };
  assert.equal(awardIncludesRecipient(award, "carol"), false);
});

test("awardIncludesRecipient handles missing tags and pubkey inputs", () => {
  assert.equal(awardIncludesRecipient(null, "alice"), false);
  assert.equal(awardIncludesRecipient({}, "alice"), false);
  assert.equal(awardIncludesRecipient({ tags: [["p", "alice"]] }, ""), false);
  assert.equal(
    awardIncludesRecipient({ tags: [["p", "alice"]] }, undefined),
    false
  );
});

test("buildBadgeViewerCollectionState returns logged-out without a signer", () => {
  assert.deepEqual(
    buildBadgeViewerCollectionState({
      signerPubkey: null,
      badgeCoordinate: "30009:issuer:day",
      awards: [],
      profileEvent: null,
    }),
    { status: "logged-out" }
  );
});

test("buildBadgeViewerCollectionState returns not-awarded when signer has no matching award", () => {
  const awards = [
    {
      id: "award-1",
      tags: [
        ["a", "30009:issuer:day"],
        ["p", "someone-else"],
      ],
    },
  ];
  assert.deepEqual(
    buildBadgeViewerCollectionState({
      signerPubkey: "viewer",
      badgeCoordinate: "30009:issuer:day",
      awards,
      profileEvent: null,
    }),
    { status: "not-awarded" }
  );
});

test("buildBadgeViewerCollectionState returns awarded when viewer has award but profile lacks pair", () => {
  const award = {
    id: "award-1",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "viewer"],
    ],
  };
  const result = buildBadgeViewerCollectionState({
    signerPubkey: "viewer",
    badgeCoordinate: "30009:issuer:day",
    awards: [award],
    profileEvent: { tags: [["d", "profile_badges"]] },
  });
  assert.deepEqual(result, { status: "awarded", award });
});

test("buildBadgeViewerCollectionState returns accepted when profile pair matches coordinate and award", () => {
  const award = {
    id: "award-1",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "viewer"],
    ],
  };
  const profileEvent = {
    tags: [
      ["d", "profile_badges"],
      ["a", "30009:issuer:day"],
      ["e", "award-1"],
    ],
  };
  const result = buildBadgeViewerCollectionState({
    signerPubkey: "viewer",
    badgeCoordinate: "30009:issuer:day",
    awards: [award],
    profileEvent,
  });
  assert.deepEqual(result, {
    status: "accepted",
    award,
    pair: {
      a: "30009:issuer:day",
      aRelay: undefined,
      e: "award-1",
      eRelay: undefined,
    },
  });
});

test("buildBadgeViewerCollectionState picks the award that includes the viewer", () => {
  const otherAward = {
    id: "award-other",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "someone-else"],
    ],
  };
  const viewerAward = {
    id: "award-viewer",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "viewer"],
    ],
  };
  const result = buildBadgeViewerCollectionState({
    signerPubkey: "viewer",
    badgeCoordinate: "30009:issuer:day",
    awards: [otherAward, viewerAward],
    profileEvent: null,
  });
  assert.deepEqual(result, { status: "awarded", award: viewerAward });
});

test("buildBadgeViewerCollectionState ignores profile pair when award id does not match", () => {
  const award = {
    id: "award-1",
    tags: [
      ["a", "30009:issuer:day"],
      ["p", "viewer"],
    ],
  };
  const profileEvent = {
    tags: [
      ["d", "profile_badges"],
      ["a", "30009:issuer:day"],
      ["e", "different-award-id"],
    ],
  };
  const result = buildBadgeViewerCollectionState({
    signerPubkey: "viewer",
    badgeCoordinate: "30009:issuer:day",
    awards: [award],
    profileEvent,
  });
  assert.deepEqual(result, { status: "awarded", award });
});

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
  buildEditedBadgeDefinitionEvent,
  buildFollowAwardeesEvent,
  buildNewBadgePreviewModel,
  canAwardBadge,
  coordinatePathFromBadge,
  deriveBadgeSlug,
  buildHideProfileBadgesEvent,
  extractAwardeePubkeys,
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

test("buildEditedBadgeDefinitionEvent preserves existing identifier", () => {
  const existing = {
    kind: 30009,
    pubkey: "author",
    created_at: 100,
    tags: [
      ["d", "scene-stealer"],
      ["name", "Old name"],
      ["description", "Old desc"],
      ["image", "https://old.example/img"],
      ["thumb", "https://old.example/thumb"],
      ["stale", "whatever"], // unrelated tag
    ],
    content: "ignore",
  };
  const edited = buildEditedBadgeDefinitionEvent({
    existingEvent: existing,
    pubkey: "author",
    name: "New name",
    description: "New desc",
    imageUrl: "https://new.example/img",
    thumbUrl: "https://new.example/thumb",
    createdAt: 200,
  });
  assert.equal(edited.kind, 30009);
  assert.equal(edited.pubkey, "author");
  assert.equal(edited.content, "");
  assert.equal(edited.created_at, 200);
  // d is preserved
  assert.deepEqual(edited.tags[0], ["d", "scene-stealer"]);
  // name/description/image/thumb are replaced with new values
  assert.equal(edited.tags.find((t) => t[0] === "name")?.[1], "New name");
  assert.equal(edited.tags.find((t) => t[0] === "description")?.[1], "New desc");
  assert.equal(edited.tags.find((t) => t[0] === "image")?.[1], "https://new.example/img");
  assert.equal(edited.tags.find((t) => t[0] === "thumb")?.[1], "https://new.example/thumb");
  // each of those tags appears exactly once (no duplication)
  ["name", "description", "image", "thumb", "d"].forEach((key) => {
    assert.equal(edited.tags.filter((t) => t[0] === key).length, 1, `exactly one ${key} tag`);
  });
});

test("buildEditedBadgeDefinitionEvent rejects missing existing event", () => {
  assert.throws(() => buildEditedBadgeDefinitionEvent({ pubkey: "a", name: "b", createdAt: 1 }), /existing/i);
  assert.throws(() => buildEditedBadgeDefinitionEvent({ existingEvent: null, pubkey: "a", name: "b", createdAt: 1 }), /existing/i);
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

test("extractAwardeePubkeys returns [] for empty or missing input", () => {
  assert.deepEqual(extractAwardeePubkeys([]), []);
  assert.deepEqual(extractAwardeePubkeys(undefined), []);
  assert.deepEqual(extractAwardeePubkeys(null), []);
});

test("extractAwardeePubkeys deduplicates p-tag values across awards", () => {
  const awards = [
    {
      id: "award-1",
      tags: [
        ["a", "30009:issuer:day"],
        ["p", "alice"],
        ["p", "bob"],
      ],
    },
    {
      id: "award-2",
      tags: [
        ["a", "30009:issuer:day"],
        ["p", "bob"],
        ["p", "carol"],
      ],
    },
  ];
  assert.deepEqual(extractAwardeePubkeys(awards), ["alice", "bob", "carol"]);
});

test("extractAwardeePubkeys returns [] when no awards have p tags", () => {
  const awards = [
    {
      id: "award-1",
      tags: [["a", "30009:issuer:day"]],
    },
    {
      id: "award-2",
      tags: [],
    },
    {
      id: "award-3",
    },
  ];
  assert.deepEqual(extractAwardeePubkeys(awards), []);
});

test("buildFollowAwardeesEvent throws when contactListEvent is null", () => {
  assert.throws(
    () =>
      buildFollowAwardeesEvent({
        pubkey: "user",
        contactListEvent: null,
        awardeePubkeys: ["alice"],
        createdAt: 1,
      }),
    /contactListEvent/i
  );
});

test("buildFollowAwardeesEvent throws when contactListEvent is undefined", () => {
  assert.throws(
    () =>
      buildFollowAwardeesEvent({
        pubkey: "user",
        contactListEvent: undefined,
        awardeePubkeys: ["alice"],
        createdAt: 1,
      }),
    /contactListEvent/i
  );
});

test("buildFollowAwardeesEvent preserves existing p tags in order and appends new awardees", () => {
  const contactListEvent = {
    kind: 3,
    pubkey: "user",
    content: "",
    tags: [
      ["p", "existing-1"],
      ["p", "existing-2"],
    ],
  };
  const event = buildFollowAwardeesEvent({
    pubkey: "user",
    contactListEvent,
    awardeePubkeys: ["new-1", "new-2"],
    createdAt: 100,
  });
  assert.equal(event.kind, 3);
  assert.equal(event.pubkey, "user");
  assert.equal(event.created_at, 100);
  assert.deepEqual(event.tags, [
    ["p", "existing-1"],
    ["p", "existing-2"],
    ["p", "new-1"],
    ["p", "new-2"],
  ]);
});

test("buildFollowAwardeesEvent dedupes awardees present in existing follows", () => {
  const contactListEvent = {
    kind: 3,
    pubkey: "user",
    content: "",
    tags: [
      ["p", "alice"],
      ["p", "bob"],
    ],
  };
  const event = buildFollowAwardeesEvent({
    pubkey: "user",
    contactListEvent,
    awardeePubkeys: ["alice", "carol", "carol", "bob"],
    createdAt: 200,
  });
  assert.deepEqual(event.tags, [
    ["p", "alice"],
    ["p", "bob"],
    ["p", "carol"],
  ]);
});

test("buildFollowAwardeesEvent preserves non-p tags after p tags", () => {
  const contactListEvent = {
    kind: 3,
    pubkey: "user",
    content: "",
    tags: [
      ["p", "alice"],
      ["t", "topic"],
      ["p", "bob"],
      ["r", "wss://relay.example"],
    ],
  };
  const event = buildFollowAwardeesEvent({
    pubkey: "user",
    contactListEvent,
    awardeePubkeys: ["carol"],
    createdAt: 300,
  });
  assert.deepEqual(event.tags, [
    ["p", "alice"],
    ["p", "bob"],
    ["p", "carol"],
    ["t", "topic"],
    ["r", "wss://relay.example"],
  ]);
});

test("buildFollowAwardeesEvent uses contactListEvent.content and kind 3", () => {
  const contactListEvent = {
    kind: 3,
    pubkey: "user",
    content: "{\"wss://relay.example\":{\"read\":true,\"write\":true}}",
    tags: [],
  };
  const event = buildFollowAwardeesEvent({
    pubkey: "user",
    contactListEvent,
    awardeePubkeys: ["alice"],
    createdAt: 400,
  });
  assert.equal(event.kind, 3);
  assert.equal(event.content, "{\"wss://relay.example\":{\"read\":true,\"write\":true}}");
  assert.equal(event.created_at, 400);
  assert.deepEqual(event.tags, [["p", "alice"]]);
});

test("buildFollowAwardeesEvent defaults missing content to empty string", () => {
  const contactListEvent = {
    kind: 3,
    pubkey: "user",
    tags: [],
  };
  const event = buildFollowAwardeesEvent({
    pubkey: "user",
    contactListEvent,
    awardeePubkeys: [],
    createdAt: 500,
  });
  assert.equal(event.content, "");
  assert.deepEqual(event.tags, []);
});

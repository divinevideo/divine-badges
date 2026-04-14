import test from "node:test";
import assert from "node:assert/strict";

import {
  buildAcceptedBadgeRecords,
  buildAcceptProfileBadgesEvent,
  buildAwardedBadgeRecords,
  buildBadgeAwardEvent,
  buildHideProfileBadgesEvent,
  buildProfileBadgeTags,
  coordinateFromBadgeDefinition,
  extractProfileBadgePairs,
} from "./badges.js";

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

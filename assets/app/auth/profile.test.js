import test from "node:test";
import assert from "node:assert/strict";

import {
  buildNavProfile,
  buildPublicCreatorSummary,
  loadCreatorProfile,
} from "./profile.js";

test("buildNavProfile prefers Divine profile display fields when present", () => {
  const profile = buildNavProfile({
    pubkey: "81acbb70475b8b715c38d072ce93769ca275783d187990117ec0c01ea849bf95",
    payload: {
      profile: {
        name: "kingbach",
        display_name: "KingBach",
        picture:
          "https://storage.googleapis.com/divine-vine-archive/avatars/93/49/934940633704046592.jpg",
        nip05: "_@kingbach.divine.video",
      },
    },
  });

  assert.deepEqual(profile, {
    displayName: "KingBach",
    avatarUrl:
      "https://storage.googleapis.com/divine-vine-archive/avatars/93/49/934940633704046592.jpg",
    initials: "K",
    handle: "@kingbach",
    username: "kingbach",
    nip05: "_@kingbach.divine.video",
    about: null,
    pubkey: "81acbb70475b8b715c38d072ce93769ca275783d187990117ec0c01ea849bf95",
  });
});

test("buildNavProfile falls back to shortened pubkey when Divine profile is missing", () => {
  const profile = buildNavProfile({
    pubkey: "d95aa8fc7b53ab5d49579f4ddfb0f10f30dbb282f381c16d4c62ba1b035ae540",
    payload: {
      profile: null,
    },
  });

  assert.deepEqual(profile, {
    displayName: "d95aa8fc…5ae540",
    avatarUrl: null,
    initials: "D",
    handle: null,
    username: null,
    nip05: null,
    about: null,
    pubkey: "d95aa8fc7b53ab5d49579f4ddfb0f10f30dbb282f381c16d4c62ba1b035ae540",
  });
});

test("buildPublicCreatorSummary prefers human identity and Divine-facing stats", () => {
  const summary = buildPublicCreatorSummary({
    pubkey: "5ab67f7d7fed4f781008c0ec0d26c8113f9fb46094a8346246c70c75e75db9fb",
    payload: {
      profile: {
        name: "improvising",
        display_name: "improvising",
        about: "I’m Nate Smith and I am improvising",
        picture: "https://media.divine.video/avatar.png",
        nip05: "_@improvising.divine.video",
      },
      stats: {
        video_count: 119,
      },
      social: {
        follower_count: 88,
      },
      engagement: {
        total_loops: 4875.383333333339,
        total_views: 5259,
      },
    },
    badgeState: {
      awarded: [{}, {}, {}],
      accepted: [{}, {}],
      created: [{}],
    },
  });

  assert.deepEqual(summary, {
    displayName: "improvising",
    avatarUrl: "https://media.divine.video/avatar.png",
    initials: "I",
    handle: "@improvising",
    username: "improvising",
    nip05: "_@improvising.divine.video",
    about: "I’m Nate Smith and I am improvising",
    pubkey: "5ab67f7d7fed4f781008c0ec0d26c8113f9fb46094a8346246c70c75e75db9fb",
    kicker: "Divine creator",
    subline: "@improvising",
    stats: [
      { label: "Videos", value: "119" },
      { label: "Followers", value: "88" },
      { label: "Loops", value: "4.9K" },
      { label: "Views", value: "5.3K" },
      { label: "Awarded", value: "3" },
      { label: "Pinned", value: "2" },
    ],
  });
});

test("loadCreatorProfile uses Divine data when the API responds", async () => {
  const divinePayload = {
    profile: {
      display_name: "Divine Name",
      picture: "https://divine.example/pic.png",
      nip05: "_@divinehandle.divine.video",
      about: "divine bio",
    },
  };
  const fetchDivineFn = async () => divinePayload;
  const loadNostrFn = async () => {
    throw new Error("nostr should be ignored when Divine has full data");
  };
  const profile = await loadCreatorProfile("pubkeyA", {
    relays: ["wss://r"],
    fetchDivineFn,
    loadNostrFn: async () => null,
  });
  assert.equal(profile.displayName, "Divine Name");
  assert.equal(profile.avatarUrl, "https://divine.example/pic.png");
  assert.equal(profile.handle, "@divinehandle");
  assert.equal(profile.nip05, "_@divinehandle.divine.video");
  assert.equal(profile.about, "divine bio");
});

test("loadCreatorProfile falls back to Nostr kind:0 when Divine API fails", async () => {
  const fetchDivineFn = async () => null;
  const loadNostrFn = async () => ({
    pubkey: "pubkeyB",
    displayName: "NostrName",
    avatarUrl: "https://nostr.example/pic.png",
    nip05: "_@nostrhandle.divine.video",
    handle: "nostrhandle",
    about: "nostr bio",
    raw: {},
    createdAt: 1,
  });
  const profile = await loadCreatorProfile("pubkeyB", {
    relays: ["wss://r"],
    fetchDivineFn,
    loadNostrFn,
  });
  assert.equal(profile.displayName, "NostrName");
  assert.equal(profile.avatarUrl, "https://nostr.example/pic.png");
  assert.equal(profile.nip05, "_@nostrhandle.divine.video");
  assert.equal(profile.handle, "@nostrhandle");
  assert.equal(profile.username, "nostrhandle");
  assert.equal(profile.about, "nostr bio");
});

test("loadCreatorProfile returns shortened pubkey fallback when both fail", async () => {
  const fetchDivineFn = async () => null;
  const loadNostrFn = async () => null;
  const profile = await loadCreatorProfile(
    "d95aa8fc7b53ab5d49579f4ddfb0f10f30dbb282f381c16d4c62ba1b035ae540",
    {
      relays: ["wss://r"],
      fetchDivineFn,
      loadNostrFn,
    }
  );
  assert.equal(profile.displayName, "d95aa8fc…5ae540");
  assert.equal(profile.avatarUrl, null);
  assert.equal(profile.handle, null);
  assert.equal(profile.nip05, null);
  assert.equal(profile.about, null);
});

test("loadCreatorProfile fills missing nip05/handle from Nostr when Divine lacks them", async () => {
  const fetchDivineFn = async () => ({
    profile: {
      display_name: "DivineName",
      picture: "https://divine.example/pic.png",
    },
  });
  const loadNostrFn = async () => ({
    pubkey: "pubkeyC",
    displayName: "NostrName",
    avatarUrl: "https://nostr.example/other.png",
    nip05: "_@nostrhandle.divine.video",
    handle: "nostrhandle",
    about: "nostr bio",
    raw: {},
    createdAt: 1,
  });
  const profile = await loadCreatorProfile("pubkeyC", {
    relays: ["wss://r"],
    fetchDivineFn,
    loadNostrFn,
  });
  assert.equal(profile.displayName, "DivineName");
  assert.equal(profile.avatarUrl, "https://divine.example/pic.png");
  assert.equal(profile.nip05, "_@nostrhandle.divine.video");
  assert.equal(profile.handle, "@nostrhandle");
  assert.equal(profile.username, "nostrhandle");
  assert.equal(profile.about, "nostr bio");
});

test("loadCreatorProfile keeps Divine populated fields and only fills gaps from Nostr", async () => {
  const fetchDivineFn = async () => ({
    profile: {
      display_name: "DivineName",
      nip05: "_@divinehandle.divine.video",
    },
  });
  const loadNostrFn = async () => ({
    pubkey: "pubkeyD",
    displayName: "NostrName",
    avatarUrl: "https://nostr.example/pic.png",
    nip05: "_@nostrhandle.divine.video",
    handle: "nostrhandle",
    about: "nostr bio",
    raw: {},
    createdAt: 1,
  });
  const profile = await loadCreatorProfile("pubkeyD", {
    relays: ["wss://r"],
    fetchDivineFn,
    loadNostrFn,
  });
  assert.equal(profile.displayName, "DivineName");
  assert.equal(profile.nip05, "_@divinehandle.divine.video");
  assert.equal(profile.handle, "@divinehandle");
  assert.equal(profile.avatarUrl, "https://nostr.example/pic.png");
  assert.equal(profile.about, "nostr bio");
});

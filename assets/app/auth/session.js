import {
  NsecSigner,
  ExtensionSigner,
  BunkerNIP44Signer,
  OAuthSigner,
  buildOAuthUrl,
  exchangeCode,
  createSessionStore,
  restoreSession,
} from "https://esm.sh/divine-signer@0.4.2";

const SESSION_KEY = "dbdg_session";
const LOGGED_OUT_KEY = "dbdg_logged_out";

const oauthStorage = {
  savePkceState: (state) => localStorage.setItem("dbdg_pkce", JSON.stringify(state)),
  loadPkceState: () => {
    try {
      return JSON.parse(localStorage.getItem("dbdg_pkce"));
    } catch {
      return null;
    }
  },
  clearPkceState: () => localStorage.removeItem("dbdg_pkce"),
  saveAuthorizationHandle: (handle) =>
    localStorage.setItem("dbdg_auth_handle", handle),
  loadAuthorizationHandle: () => localStorage.getItem("dbdg_auth_handle"),
  clearAuthorizationHandle: () => localStorage.removeItem("dbdg_auth_handle"),
};

const oauthConfig = {
  clientId: "divine-badges",
  redirectUri: /\.divine\.video$/.test(window.location.hostname)
    ? "https://badges.divine.video/me"
    : `${window.location.origin}/me`,
  storage: oauthStorage,
};

const sessions = createSessionStore(localStorage, SESSION_KEY);

function readDivineCookie(name) {
  const match = document.cookie.match(new RegExp(`(?:^|; )${name}=([^;]+)`));
  if (!match) {
    return null;
  }
  try {
    return JSON.parse(atob(decodeURIComponent(match[1])));
  } catch {
    return null;
  }
}

function sharedDivineSession() {
  if (!/\.divine\.video$/.test(window.location.hostname)) {
    return null;
  }
  const login = readDivineCookie("nostr_login");
  const jwt = readDivineCookie("divine_jwt");
  const bunkerUrl = login?.bunkerUri || jwt?.bunkerUrl;
  if (login?.type === "bunker" && bunkerUrl) {
    return { type: "bunker", bunkerUrl };
  }
  if (login?.type === "extension") {
    return { type: "extension" };
  }
  if (bunkerUrl) {
    return { type: "bunker", bunkerUrl };
  }
  return null;
}

function attachRefreshPersistence(signer) {
  if (signer instanceof OAuthSigner) {
    signer.onTokenRefresh = ({ accessToken, refreshToken }) => {
      sessions.save({ type: "oauth", accessToken, refreshToken });
    };
  }
}

export async function beginDivineOAuth() {
  return buildOAuthUrl(oauthConfig);
}

export async function loginWithExtension() {
  const signer = new ExtensionSigner();
  await signer.getPublicKey();
  sessions.save({ type: "extension" });
  attachRefreshPersistence(signer);
  return signer;
}

export async function loginWithBunker(bunkerUrl) {
  const signer = await BunkerNIP44Signer.fromBunkerUrl(bunkerUrl);
  sessions.save({ type: "bunker", bunkerUrl });
  attachRefreshPersistence(signer);
  return signer;
}

export async function loginWithNsec(nsec) {
  const signer = new NsecSigner(nsec);
  await signer.getPublicKey();
  sessions.save({ type: "nsec", nsec });
  attachRefreshPersistence(signer);
  return signer;
}

export function markSessionActive() {
  localStorage.removeItem(LOGGED_OUT_KEY);
}

export function clearStoredSession() {
  sessions.clear();
  localStorage.setItem(LOGGED_OUT_KEY, "1");
}

export async function bootstrapSession() {
  const params = new URLSearchParams(window.location.search);
  const code = params.get("code");
  const state = params.get("state");
  if (code && state) {
    const { signer, accessToken, refreshToken } = await exchangeCode(
      code,
      state,
      oauthConfig
    );
    sessions.save({ type: "oauth", accessToken, refreshToken });
    window.history.replaceState({}, "", window.location.pathname);
    attachRefreshPersistence(signer);
    return signer;
  }

  const stored = sessions.load();
  if (stored) {
    try {
      const signer = await restoreSession(stored);
      attachRefreshPersistence(signer);
      return signer;
    } catch {
      sessions.clear();
    }
  }

  const explicitlyLoggedOut = localStorage.getItem(LOGGED_OUT_KEY) === "1";
  if (!explicitlyLoggedOut) {
    const shared = sharedDivineSession();
    if (shared) {
      try {
        const signer = await restoreSession(shared);
        sessions.save(shared);
        attachRefreshPersistence(signer);
        return signer;
      } catch {}
    }
  }

  return null;
}

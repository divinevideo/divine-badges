const RETURN_TO_KEY = "dbdg_return_to";
const DEFAULT_RETURN_TO = "/me";

export function sanitizeReturnTo(value) {
  if (typeof value !== "string") {
    return null;
  }
  if (!value.startsWith("/") || value.startsWith("//") || value.includes("\\")) {
    return null;
  }
  return value;
}

export function saveReturnTo(path, storage = window.sessionStorage) {
  const safe = sanitizeReturnTo(path);
  if (safe) {
    storage.setItem(RETURN_TO_KEY, safe);
  } else {
    storage.removeItem(RETURN_TO_KEY);
  }
}

export function consumeReturnTo(storage = window.sessionStorage) {
  const value = storage.getItem(RETURN_TO_KEY);
  storage.removeItem(RETURN_TO_KEY);
  return sanitizeReturnTo(value) ?? DEFAULT_RETURN_TO;
}

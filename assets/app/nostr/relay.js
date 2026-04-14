export function newestFirst(events) {
  return [...events].sort((left, right) => right.created_at - left.created_at);
}

export function relayQuery(relayUrl, filters, timeoutMs = 6000) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl);
    const subId = "r" + Math.random().toString(36).slice(2, 10);
    const events = [];
    const done = (value) => {
      clearTimeout(timer);
      try {
        ws.close();
      } catch {}
      resolve(value);
    };
    const timer = setTimeout(() => done(events), timeoutMs);
    ws.onopen = () => ws.send(JSON.stringify(["REQ", subId, ...filters]));
    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message[0] === "EVENT" && message[1] === subId) {
          events.push(message[2]);
        } else if (message[0] === "EOSE" && message[1] === subId) {
          done(events);
        }
      } catch {}
    };
    ws.onerror = () => {
      clearTimeout(timer);
      reject(new Error("relay error"));
    };
  });
}

export function relayPublish(relayUrl, nostrEvent, timeoutMs = 8000) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(relayUrl);
    const timer = setTimeout(() => {
      try {
        ws.close();
      } catch {}
      reject(new Error("relay timeout"));
    }, timeoutMs);
    ws.onopen = () => ws.send(JSON.stringify(["EVENT", nostrEvent]));
    ws.onmessage = (event) => {
      try {
        const message = JSON.parse(event.data);
        if (message[0] === "OK" && message[1] === nostrEvent.id) {
          clearTimeout(timer);
          try {
            ws.close();
          } catch {}
          if (message[2]) {
            resolve();
          } else {
            reject(new Error(message[3] || "relay rejected event"));
          }
        }
      } catch {}
    };
    ws.onerror = () => {
      clearTimeout(timer);
      reject(new Error("relay error"));
    };
  });
}

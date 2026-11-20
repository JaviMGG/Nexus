import { apiUrl } from "./api";
import type { AppEvent } from "./types";

type Listener = (event: AppEvent) => void;

const listeners = new Set<Listener>();
let source: EventSource | null = null;
let retryDelay = 1000;
let started = false;

async function connect() {
  const url = await apiUrl("/api/events");
  source = new EventSource(url);
  source.onopen = () => {
    retryDelay = 1000;
  };
  source.onmessage = (msg) => {
    try {
      const event = JSON.parse(msg.data) as AppEvent;
      listeners.forEach((l) => l(event));
    } catch {
      // evento malformado: ignorar
    }
  };
  source.onerror = () => {
    source?.close();
    source = null;
    setTimeout(connect, retryDelay);
    retryDelay = Math.min(retryDelay * 2, 15000);
  };
}

export function startEvents() {
  if (started) return;
  started = true;
  void connect();
}

export function subscribeEvents(listener: Listener): () => void {
  listeners.add(listener);
  startEvents();
  return () => {
    listeners.delete(listener);
  };
}

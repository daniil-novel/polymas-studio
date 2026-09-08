import { afterAll } from 'vitest';

// Node 25 defines Web Storage globals before Vitest populates JSDOM's window.
// Use this test environment's real browser Storage, not Node's file-backed global.
const environment = globalThis as typeof globalThis & { jsdom: { window: Window } };
for (const name of ['localStorage', 'sessionStorage'] as const) {
  const original = Object.getOwnPropertyDescriptor(globalThis, name);
  Object.defineProperty(globalThis, name, {
    configurable: true,
    value: environment.jsdom.window[name],
  });
  afterAll(() => {
    if (original) Object.defineProperty(globalThis, name, original);
    else Reflect.deleteProperty(globalThis, name);
  });
}

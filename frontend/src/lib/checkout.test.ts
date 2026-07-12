import { describe, it, expect, afterEach, vi } from 'vitest';

// checkout.ts transitively imports systemService -> @tauri-apps/api/core; stub it
// so the module graph loads in the node test env (openCheckout is not exercised here).
vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const ENV_KEY = 'NEXT_PUBLIC_MITYU_CHECKOUT_URL';
const FALLBACK = 'https://mityu.vercel.app/#pricing';

describe('CHECKOUT_URL resolution', () => {
  const original = process.env[ENV_KEY];

  afterEach(() => {
    if (original === undefined) delete process.env[ENV_KEY];
    else process.env[ENV_KEY] = original;
    vi.resetModules();
  });

  it('falls back to the live pricing page when the env var is unset', async () => {
    delete process.env[ENV_KEY];
    vi.resetModules();
    const { CHECKOUT_URL } = await import('./checkout');
    expect(CHECKOUT_URL).toBe(FALLBACK);
  });

  it('uses the injected checkout URL when set', async () => {
    process.env[ENV_KEY] = 'https://buy.polar.sh/some-product';
    vi.resetModules();
    const { CHECKOUT_URL } = await import('./checkout');
    expect(CHECKOUT_URL).toBe('https://buy.polar.sh/some-product');
  });

  it('trims and falls back when the injected value is blank', async () => {
    process.env[ENV_KEY] = '   ';
    vi.resetModules();
    const { CHECKOUT_URL } = await import('./checkout');
    expect(CHECKOUT_URL).toBe(FALLBACK);
  });

  it('openCheckout is a no-op (no throw) outside a browser/Tauri env', async () => {
    delete process.env[ENV_KEY];
    vi.resetModules();
    const { openCheckout } = await import('./checkout');
    expect(() => openCheckout()).not.toThrow();
  });
});

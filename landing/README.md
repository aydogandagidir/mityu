# Landing deployment configuration

The anonymous download endpoint accepts no contact or marketing fields. Its optional Upstash/Vercel KV path needs a complete server-side configuration:

- `KV_REST_API_URL` and `KV_REST_API_TOKEN`, or the equivalent `UPSTASH_REDIS_REST_URL` and `UPSTASH_REDIS_REST_TOKEN`
- `DOWNLOAD_RATE_LIMIT_HMAC_SECRET`: a cryptographically random value of at least 32 bytes

Store the HMAC value as an encrypted Vercel environment secret for every environment that has KV enabled. It must never use a `NEXT_PUBLIC_` name, be sent to the browser, or be committed to source. Generate a new value with a cryptographically secure random generator; do not reuse a user password, API token, or signing key.

If KV is absent, the endpoint writes no counter and stores no IP-derived identifier. If KV is present but any credential or the HMAC secret is missing or invalid, the endpoint returns `503` before any KV write; it never falls back to raw IP storage or an unkeyed hash. The page still starts the installer download independently of this best-effort counter response.

The rate limiter stores only a keyed HMAC token and count. Its atomic Redis script applies the 15-minute TTL when the counter is first created; subsequent requests do not extend that fixed window.

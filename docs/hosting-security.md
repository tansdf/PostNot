# Marketing site security headers

The public site is served from `docs/` through GitHub Pages and proxied by Cloudflare. GitHub Pages does not provide repository-controlled response headers, and HTML `<meta>` elements cannot enforce every relevant policy—most importantly `frame-ancestors`.

Configure these as Cloudflare response headers when the zone configuration is available:

```text
Content-Security-Policy: default-src 'self'; connect-src 'self' https://api.github.com; img-src 'self' data:; script-src 'self'; style-src 'self'; font-src 'self'; base-uri 'self'; form-action 'self'; frame-ancestors 'none'; object-src 'none'; upgrade-insecure-requests
Referrer-Policy: strict-origin-when-cross-origin
X-Content-Type-Options: nosniff
Permissions-Policy: camera=(), geolocation=(), microphone=(), payment=(), usb=()
```

Do not enable Cloudflare browser analytics unless the product decision changes. If it is enabled, its injected beacon must be deliberately added to `script-src` and `connect-src`; the site itself includes no analytics or tracking code.

After changing the zone, verify the production response with:

```bash
curl -I https://post-not.com/
```

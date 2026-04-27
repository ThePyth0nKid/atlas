import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Disable runtime image optimisation — Atlas UI has no <Image>.
  images: { unoptimized: true },

  // Allow loading our WASM module from /public/wasm.
  // Next.js 16 serves anything in /public verbatim, but we set headers
  // so the browser caches the WASM aggressively (it's content-hashed via the bundle hash).
  async headers() {
    return [
      {
        source: "/wasm/:path*",
        headers: [
          { key: "Cross-Origin-Embedder-Policy", value: "require-corp" },
          { key: "Cross-Origin-Opener-Policy", value: "same-origin" },
          { key: "Cache-Control", value: "public, max-age=3600, must-revalidate" },
        ],
      },
    ];
  },

  // react-force-graph imports d3 ESM modules that some Webpack/Turbopack versions
  // mishandle in SSR. Mark these as client-only via dynamic-import in the component.
};

export default nextConfig;

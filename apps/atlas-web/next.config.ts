import type { NextConfig } from "next";

const nextConfig: NextConfig = {
  // Disable runtime image optimisation — Atlas UI has no <Image>.
  images: { unoptimized: true },

  // V1.19 Welle 2: the bridge is a workspace package whose `package.json`
  // `exports` field points at `dist/index.js`. `transpilePackages`
  // routes that resolved file through Next/Turbopack's SWC pipeline
  // instead of treating it as pre-compiled — which is what we want for
  // a freshly-built `dist/` that hasn't gone through the host's
  // transpiler. It does NOT make Next read `src/` directly; bridge
  // edits require `pnpm --filter @atlas/bridge build` to be visible
  // here. (Adding `"source": "./src/index.ts"` to the bridge's
  // package.json would change that contract, but we deliberately keep
  // the source/dist boundary explicit so the e2e smoke tests the
  // shipped artefact.)
  transpilePackages: ["@atlas/bridge"],

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

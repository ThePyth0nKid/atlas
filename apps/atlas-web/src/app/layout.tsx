import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";

export const metadata: Metadata = {
  title: "Atlas — Verifiable Knowledge Graphs",
  description:
    "Knowledge graphs the agent can prove, not just claim. Ed25519 + COSE_Sign1 + Sigstore Rekor + WASM verifier in your browser.",
};

const NAV = [
  { href: "/", label: "Audit Readiness" },
  { href: "/graph", label: "Knowledge Graph" },
  { href: "/write", label: "Write" },
  { href: "/compliance", label: "Compliance Lens" },
  { href: "/audit-export", label: "Audit Export" },
  { href: "/adversary-demo", label: "Adversary Demo" },
];

export default function RootLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <html lang="en" className="h-full">
      <body className="min-h-full bg-[var(--background)] text-[var(--foreground)]">
        <header className="h-12 border-b border-[var(--border)] flex items-center px-6 gap-6">
          <Link href="/" className="font-semibold tracking-tight">
            Atlas
          </Link>
          <nav className="flex items-center gap-5 text-[13px] text-[var(--foreground-muted)]">
            {NAV.map((item) => (
              <Link
                key={item.href}
                href={item.href}
                className="hover:text-[var(--foreground)] transition-colors"
              >
                {item.label}
              </Link>
            ))}
          </nav>
          {/*
            V1.19 Welle 1: removed the hardcoded "ws-bankhaus-hagedorn"
            chip and fake "last anchor 47s ago" indicator. They were
            a static demo affordance that misrepresented system state
            on every page (the chip has no relation to the workspace
            actually being viewed; the timestamp ticked nothing). A
            future welle wires a real workspace-selector + last-anchor
            ticker driven by the verifier output. Until then, omit.
          */}
        </header>
        <main className="max-w-[1280px] mx-auto px-8 py-8">{children}</main>
      </body>
    </html>
  );
}

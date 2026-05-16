import type { Metadata } from "next";
import Link from "next/link";
import "./globals.css";
import { WorkspaceProvider } from "@/lib/workspace-context";
import { WorkspaceSelector } from "@/components/WorkspaceSelector";

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
        <WorkspaceProvider>
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
              W20a: real workspace selector (replaces V1.19 Welle 1's
              omission of the hardcoded "ws-bankhaus-hagedorn" chip).
              The selector reads from `WorkspaceProvider` and drives
              `LiveVerifierPanel` + `KnowledgeGraphView` to fetch the
              currently-selected workspace's trace instead of the
              golden bank-demo fixture.
            */}
            <div className="ml-auto">
              <WorkspaceSelector />
            </div>
          </header>
          <main className="max-w-[1280px] mx-auto px-8 py-8">{children}</main>
        </WorkspaceProvider>
      </body>
    </html>
  );
}

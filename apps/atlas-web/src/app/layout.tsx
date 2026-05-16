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

/**
 * W20b-1 — nav model.
 *
 * `kind: "link"` entries are routable today. `kind: "soon"` entries
 * render as a disabled span with a tooltip explaining the welle in
 * which they ship. This is the honest UI alternative to letting users
 * click into a 404. Each coming-soon entry carries a unique testid
 * suffix so `tests/e2e/dashboard-tiers.spec.ts` can assert the
 * disabled state without coupling to label text.
 *
 * The `kind: "showcase"` entry is the new `/demo/bank` route — it is
 * a real route but visually de-emphasised so users read it as
 * "marketing demo" rather than "core app feature".
 */
type NavItem =
  | { kind: "link"; href: string; label: string }
  | { kind: "soon"; label: string; testid: string }
  | { kind: "showcase"; href: string; label: string };

const NAV: ReadonlyArray<NavItem> = [
  { kind: "link", href: "/", label: "Audit Readiness" },
  { kind: "link", href: "/graph", label: "Knowledge Graph" },
  { kind: "link", href: "/write", label: "Write" },
  // W20c — new /settings route lit up here. Sits between core
  // surfaces and coming-soon roadmap entries so operator-side
  // controls are findable without lurking next to the bank-demo
  // showcase chip.
  { kind: "link", href: "/settings", label: "Settings" },
  {
    kind: "soon",
    label: "Compliance Lens",
    testid: "nav-coming-soon-compliance",
  },
  {
    kind: "soon",
    label: "Audit Export",
    testid: "nav-coming-soon-audit-export",
  },
  {
    kind: "soon",
    label: "Adversary Demo",
    testid: "nav-coming-soon-adversary-demo",
  },
  { kind: "showcase", href: "/demo/bank", label: "Bank demo" },
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
              {NAV.map((item) => {
                if (item.kind === "link") {
                  return (
                    <Link
                      key={item.href}
                      href={item.href}
                      className="hover:text-[var(--foreground)] transition-colors"
                    >
                      {item.label}
                    </Link>
                  );
                }
                if (item.kind === "showcase") {
                  // W20b-1 fix-commit (CRIT a11y): removed `opacity-70`
                  // (+ paired `hover:opacity-100`) that previously made
                  // the inherited `--foreground-muted` (#475569) compute
                  // to 3.59:1 contrast on the white nav — below WCAG
                  // 2.1 AA's 4.5:1 threshold. Visual de-emphasis is now
                  // carried purely by `text-[12px] uppercase
                  // tracking-wide`, which already reads as a marketing
                  // showcase chip without dimming the foreground color
                  // below AA.
                  return (
                    <Link
                      key={item.href}
                      href={item.href}
                      className="text-[12px] uppercase tracking-wide hover:text-[var(--foreground)] transition-colors"
                      data-testid="nav-showcase-bank-demo"
                    >
                      {item.label}
                    </Link>
                  );
                }
                // kind === "soon"
                // W20b-1 fix-commit (HIGH a11y): added `role="button"`
                // so screen-reader users see the disabled control. Bare
                // `aria-disabled` on a non-interactive <span> would be
                // silently dropped by AT trees. The roadmap copy is
                // intentionally welle-agnostic (was "Coming in W20c–
                // W30") so the public UI does not commit to internal
                // codenames that may shift — see security-reviewer
                // TM-W20b-7.
                return (
                  <span
                    key={item.testid}
                    className="text-[var(--foreground-muted)] cursor-not-allowed opacity-60"
                    role="button"
                    title="Coming soon — see Atlas roadmap"
                    aria-disabled="true"
                    data-testid={item.testid}
                  >
                    {item.label}
                  </span>
                );
              })}
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

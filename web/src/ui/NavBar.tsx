import { useEffect, useRef, useState } from "react";
import { hrefFor, navigate, type Route } from "../router";
import { store, type AppState } from "../store";

/** Icônes des onglets : visibles seulement dans la barre du bas, sur téléphone. */
const TABS: { name: Route["name"]; label: string; icon: string }[] = [
  { name: "home", label: "Jouer", icon: "M8 5.5v13l10-6.5z" },
  { name: "live", label: "En direct", icon: "M12 9.5a2.5 2.5 0 110 5 2.5 2.5 0 010-5zM7.5 7.5a6.4 6.4 0 000 9M16.5 7.5a6.4 6.4 0 010 9M4.6 4.6a10.5 10.5 0 000 14.8M19.4 4.6a10.5 10.5 0 010 14.8" },
  { name: "ranking", label: "Classement", icon: "M4 20V11M10 20V5M16 20v-7M21 20H3" },
  { name: "friends", label: "Amis", icon: "M9 4.5a3.5 3.5 0 110 7 3.5 3.5 0 010-7zM2.5 20c.8-3.6 3.4-5.5 6.5-5.5s5.7 1.9 6.5 5.5M16 4.8a3.5 3.5 0 010 6.4M18.5 14.8c1.6.8 2.6 2.5 3 5.2" },
  { name: "collection", label: "Collection", icon: "M5 6h11v15H5zM8 3h11a1 1 0 011 1v14" },
];

export function Wordmark() {
  return (
    <a className="wordmark" href="#/" aria-label="Chessy, accueil">
      {/* Couronne « Crown5 » de Reicon (MIT) dans un cartouche à liseré d'accent. */}
      <span className="wordmark-mark" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="20" height="20" focusable="false" fill="currentColor">
          <path d="M17 22H7a.75.75 0 010-1.5h10a.75.75 0 010 1.5z" />
          <path d="M20.35 5.52l-4 2.86c-.53.38-1.29.15-1.52-.46l-1.89-5.04c-.32-.87-1.55-.87-1.87 0l-1.9 5.03c-.23.62-.98.85-1.51.46l-4-2.86c-.8-.56-1.86.23-1.53 1.16l4.16 11.65c.14.4.52.66.94.66h9.53c.42 0 .8-.27.94-.66l4.16-11.65c.34-.93-.72-1.72-1.51-1.16zM14.5 14.75h-5a.75.75 0 010-1.5h5a.75.75 0 010 1.5z" />
        </svg>
      </span>
      <span>Chessy</span>
    </a>
  );
}

export function initialOf(name: string | null | undefined): string {
  return ((name ?? "").trim().charAt(0) || "?").toUpperCase();
}

export function NavBar({ state, route }: { state: AppState; route: Route["name"] }) {
  const { account, friends } = state;
  const pending = friends.incoming.length;
  const activeTab =
    route === "watch" ? "live" : route === "profile" || route === "auth" || route === "settings" || route === "games" || route === "replay" ? null : route;

  return (
    <header className="nav">
      <div className="nav-in">
        <Wordmark />
        <nav className="nav-tabs" aria-label="Navigation principale">
          {TABS.map((tab) => (
            <a
              key={tab.name}
              href={hrefFor({ name: tab.name })}
              className="nav-tab"
              aria-current={activeTab === tab.name ? "page" : undefined}
            >
              <svg className="nav-tab-icon" viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden="true" focusable="false">
                <path d={tab.icon} />
              </svg>
              {tab.label}
              {tab.name === "friends" && pending > 0 && (
                <span className="nav-count">
                  {pending}
                  <span className="sr-only"> demande{pending > 1 ? "s" : ""} d'ami en attente</span>
                </span>
              )}
            </a>
          ))}
        </nav>
        <div className="nav-user">
          {account && !account.guest && account.username ? (
            <UserMenu username={account.username} elo={account.elo} />
          ) : (
            account && (
              <>
                <a className="nav-gear" href={hrefFor({ name: "settings" })} aria-label="Réglages" title="Réglages">
                  <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true" focusable="false" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
                    <circle cx="12" cy="12" r="3.2" />
                    <path d="M12 2.8v2.6M12 18.6v2.6M4.2 7.4l2.2 1.3M17.6 15.3l2.2 1.3M4.2 16.6l2.2-1.3M17.6 8.7l2.2-1.3" />
                  </svg>
                </a>
                <span className="nav-guest muted">Invité</span>
                <button type="button" className="btn sm" onClick={() => navigate({ name: "auth" })}>
                  Se connecter
                </button>
              </>
            )
          )}
        </div>
      </div>
    </header>
  );
}

function UserMenu({ username, elo }: { username: string; elo: number }) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  return (
    <div className="nav-menu" ref={root}>
      <button
        type="button"
        className="nav-chip"
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        <span className="avatar sm">{initialOf(username)}</span>
        <span className="nav-chip-name">{username}</span>
        <span className="nav-chip-elo mono">{elo}</span>
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true" focusable="false">
          <path d="M2 4.5l4 4 4-4" fill="none" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      </button>
      {open && (
        <div className="nav-pop card" role="menu">
          <a
            role="menuitem"
            href={hrefFor({ name: "profile", param: username })}
            onClick={() => setOpen(false)}
          >
            Profil
          </a>
          <a role="menuitem" href={hrefFor({ name: "settings" })} onClick={() => setOpen(false)}>
            Réglages
          </a>
          <a role="menuitem" href={hrefFor({ name: "games" })} onClick={() => setOpen(false)}>
            Mes parties
          </a>
          <button
            type="button"
            role="menuitem"
            onClick={() => {
              setOpen(false);
              void store.logout();
              navigate({ name: "home" });
            }}
          >
            Déconnexion
          </button>
        </div>
      )}
    </div>
  );
}

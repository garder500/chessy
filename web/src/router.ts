import { useSyncExternalStore } from "react";

/**
 * Routes par hash : #/, #/auth, #/ranking, #/friends, #/profile/<pseudo>, #/collection,
 * #/live, #/watch/<partie>, #/games, #/replay/<partie>[/analyse], #/campaign[/<niveau>]
 */
export interface Route {
  name: "home" | "auth" | "ranking" | "friends" | "profile" | "collection" | "settings" | "live" | "watch" | "games" | "replay" | "campaign";
  param?: string;
  /** `#/replay/<id>/analyse` : ouvre le replay en lançant tout de suite l'analyse. */
  sub?: "analyse";
}

export function parseHash(hash: string): Route {
  const parts = hash.replace(/^#\/?/, "").split("/").filter(Boolean);
  const id = (i: number) => {
    try {
      return decodeURIComponent(parts[i]);
    } catch {
      return parts[i];
    }
  };
  switch (parts[0]) {
    case "auth":
      return { name: "auth" };
    case "ranking":
      return { name: "ranking" };
    case "friends":
      return { name: "friends" };
    case "collection":
      return { name: "collection" };
    case "settings":
      return { name: "settings" };
    case "live":
      return { name: "live" };
    case "games":
      return { name: "games" };
    case "campaign":
      return parts[1] ? { name: "campaign", param: id(1) } : { name: "campaign" };
    case "watch":
      return parts[1] ? { name: "watch", param: id(1) } : { name: "live" };
    case "replay":
      if (!parts[1]) return { name: "games" };
      return parts[2] === "analyse" ? { name: "replay", param: id(1), sub: "analyse" } : { name: "replay", param: id(1) };
    case "profile":
      return parts[1] ? { name: "profile", param: decodeURIComponent(parts[1]) } : { name: "home" };
    default:
      return { name: "home" };
  }
}

export function hrefFor(route: Route): string {
  switch (route.name) {
    case "home":
      return "#/";
    case "profile":
    case "watch":
      return `#/${route.name}/${encodeURIComponent(route.param ?? "")}`;
    case "replay":
      return `#/replay/${encodeURIComponent(route.param ?? "")}${route.sub === "analyse" ? "/analyse" : ""}`;
    case "campaign":
      return route.param ? `#/campaign/${encodeURIComponent(route.param)}` : "#/campaign";
    default:
      return `#/${route.name}`;
  }
}

export function navigate(route: Route) {
  location.hash = hrefFor(route);
}

function subscribe(fn: () => void) {
  window.addEventListener("hashchange", fn);
  return () => window.removeEventListener("hashchange", fn);
}

// Une chaîne est stable entre deux rendus, contrairement à un objet fraîchement construit.
const snapshot = () => location.hash;

export function useRoute(): Route {
  return parseHash(useSyncExternalStore(subscribe, snapshot));
}

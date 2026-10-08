// Client REST typé pour /api (voir docs/spec-v2.md §1). Aucune dépendance au store :
// les appelants passent le jeton de session quand la route l'exige.
import type {
  Analysis,
  ExploreRequest,
  ExploreResponse,
  GameRecord,
  Leaderboard,
  LiveGames,
  Me,
  MyGames,
  MySkills,
  PublicProfile,
} from "./protocol";

export interface AuthResponse {
  token: string;
  player: Me;
}

export interface Credentials {
  username: string;
  password: string;
}

/** Erreur normalisée : `code` est le champ `error` du serveur (ou `network` / `http_<statut>`). */
export class ApiError extends Error {
  readonly code: string;
  readonly status: number;

  constructor(code: string, status: number, message?: string) {
    super(message ?? code);
    this.name = "ApiError";
    this.code = code;
    this.status = status;
  }
}

const ERROR_TEXT: Record<string, string> = {
  username_taken: "Ce pseudo est déjà pris.",
  bad_credentials: "Pseudo ou mot de passe incorrect.",
  weak_password: "Mot de passe trop faible : 8 à 128 caractères.",
  invalid_username: "Pseudo invalide : 3 à 16 caractères, lettres, chiffres ou _.",
  unauthorized: "Votre session a expiré. Reconnectez-vous.",
  not_found: "Introuvable.",
  network: "Impossible de joindre le serveur. Vérifiez votre connexion.",
  rate_limited: "Trop de tentatives. Réessayez dans un instant.",
  too_many_attempts: "Trop d'échecs de connexion. Réessayez dans quelques minutes.",
};

/** Message français pour une erreur API (ou quelconque). */
export function apiErrorText(err: unknown): string {
  if (err instanceof ApiError) {
    if (ERROR_TEXT[err.code]) return ERROR_TEXT[err.code];
    if (err.status === 401) return ERROR_TEXT.unauthorized;
    if (err.status === 404) return ERROR_TEXT.not_found;
    if (err.status >= 500) return "Le serveur a rencontré un problème. Réessayez plus tard.";
  }
  return "Une erreur est survenue. Réessayez.";
}

import { noticeForged } from "./forged";

interface RequestOptions {
  method?: "GET" | "POST";
  body?: unknown;
  token?: string;
  signal?: AbortSignal;
}

async function request<T>(path: string, opts: RequestOptions = {}): Promise<T> {
  const headers: Record<string, string> = { Accept: "application/json" };
  if (opts.body !== undefined) headers["Content-Type"] = "application/json";
  if (opts.token) headers.Authorization = `Bearer ${opts.token}`;

  let res: Response;
  try {
    res = await fetch(`/api${path}`, {
      method: opts.method ?? "GET",
      headers,
      body: opts.body !== undefined ? JSON.stringify(opts.body) : undefined,
      signal: opts.signal,
    });
  } catch (e) {
    if (e instanceof DOMException && e.name === "AbortError") throw e;
    throw new ApiError("network", 0);
  }

  if (!res.ok) {
    let code = `http_${res.status}`;
    try {
      const data = (await res.json()) as { error?: unknown };
      if (typeof data?.error === "string") code = data.error;
    } catch {
      // Corps absent ou non JSON : on garde le code HTTP.
    }
    throw new ApiError(code, res.status);
  }
  if (res.status === 204) return undefined as T;
  try {
    const text = await res.text();
    // Rejeux, profils, parties : ils citent des compétences forgées qu'il faudra décrire.
    noticeForged(text);
    return JSON.parse(text) as T;
  } catch {
    throw new ApiError("bad_response", res.status);
  }
}

export const api = {
  register(input: Credentials & { guest_token?: string }, signal?: AbortSignal) {
    return request<AuthResponse>("/auth/register", { method: "POST", body: input, signal });
  },
  login(input: Credentials, signal?: AbortSignal) {
    return request<AuthResponse>("/auth/login", { method: "POST", body: input, signal });
  },
  logout(token: string) {
    return request<void>("/auth/logout", { method: "POST", token });
  },
  /** Termine toutes les sessions du compte (tous les appareils). */
  logoutAll(token: string) {
    return request<void>("/auth/logout-all", { method: "POST", token });
  },
  me(token: string, signal?: AbortSignal) {
    return request<Me>("/me", { token, signal });
  },
  leaderboard(limit = 50, offset = 0, signal?: AbortSignal) {
    return request<Leaderboard>(`/leaderboard?limit=${limit}&offset=${offset}`, { signal });
  },
  profile(username: string, signal?: AbortSignal) {
    return request<PublicProfile>(`/players/${encodeURIComponent(username)}`, { signal });
  },
  /** Historique de mes compétences, obtenues, forgées ou perdues (Bearer requis). */
  mySkills(token: string, signal?: AbortSignal) {
    return request<MySkills>("/me/skills", { token, signal });
  },
  // ---- v4 : parties enregistrées, replays, analyse, direct (docs/spec-v4.md §2-§3) ----
  /** Mes parties, de la plus récente à la plus ancienne (Bearer requis). */
  myGames(token: string, limit = 20, offset = 0, signal?: AbortSignal) {
    return request<MyGames>(`/me/games?limit=${limit}&offset=${offset}`, { token, signal });
  },
  /** Partie complète avec ses positions. Bearer optionnel : les parties solo ne sont lisibles que par leur joueur. */
  game(id: string, token?: string, signal?: AbortSignal) {
    return request<GameRecord>(`/games/${encodeURIComponent(id)}`, { token, signal });
  },
  /** Analyse du moteur (peut durer jusqu'à ~25 s, puis mise en cache côté serveur). */
  analysis(id: string, depth = 3, token?: string, signal?: AbortSignal) {
    return request<Analysis>(`/games/${encodeURIComponent(id)}/analysis?depth=${depth}`, { token, signal });
  },
  /** Exploration sans état : renvoie la position après `ply` actions puis `line`. */
  explore(id: string, body: ExploreRequest, token?: string, signal?: AbortSignal) {
    return request<ExploreResponse>(`/games/${encodeURIComponent(id)}/explore`, { method: "POST", body, token, signal });
  },
  /** Parties en cours (publiques), triées par Elo moyen puis ancienneté. */
  live(limit = 50, token?: string, signal?: AbortSignal) {
    return request<LiveGames>(`/live?limit=${limit}`, { token, signal });
  },
};

/** Message français pour une erreur des routes de replay, d'analyse et d'exploration. */
export function gameErrorText(err: unknown): string {
  if (err instanceof ApiError) {
    if (err.code === "no_replay") return "Replay indisponible pour cette partie.";
    if (err.status === 404 || err.code === "not_found" || err.code === "no_such_game") return "Partie introuvable.";
    if (err.status === 403) return "Cette partie n'est pas accessible avec votre compte.";
    if (err.code === "illegal_action") return "Action impossible dans cette position.";
    if (err.code === "bad_ply") return "Position introuvable dans cette partie.";
    if (err.code === "analysis_failed" || err.code === "analysis_timeout") return "L'analyse a échoué. Réessayez avec une profondeur plus faible.";
    if (err.status === 400) return "Requête invalide.";
  }
  return apiErrorText(err);
}

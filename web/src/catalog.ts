import { forgedDef, isForgedId, type Rarity } from "./forged";
import type { BuiltinSkillId, SkillId } from "./protocol";

// Catalogue des 27 compétences (source : docs/skills.md). Les identifiants sont ceux du serveur
// (`SkillId`) ; `implemented` vaut vrai pour les 27 depuis la v3.

export type Family = "attack" | "defense" | "mobility" | "control" | "create";

export type CatalogId = BuiltinSkillId;

export interface CatalogEntry {
  id: SkillId;
  name: string;
  family: Family;
  unique: boolean;
  /** Description française courte. */
  description: string;
  /** Jouable côté serveur (les 27 le sont depuis la v3). */
  implemented: boolean;
  /** Rareté d'une compétence forgée ; les 27 écrites à la main n'en ont pas. */
  rarity?: Rarity;
}

export const FAMILY_LABEL: Record<Family, string> = {
  attack: "Attaque",
  defense: "Défense",
  mobility: "Mobilité",
  control: "Contrôle",
  create: "Création",
};

export const FAMILIES: Family[] = ["attack", "defense", "mobility", "control", "create"];

/** Une fiche des 27 compétences écrites à la main. */
export type BuiltinEntry = CatalogEntry & { id: CatalogId };

const e = (
  id: CatalogId,
  name: string,
  family: Family,
  description: string,
  opts: { unique?: boolean; implemented?: boolean } = {},
): BuiltinEntry => ({
  id,
  name,
  family,
  unique: opts.unique ?? false,
  description,
  implemented: opts.implemented ?? true,
});

export const CATALOG: readonly BuiltinEntry[] = [
  // Compétences classiques
  e("teleportation", "Teleportation", "mobility", "Déplace une de vos pièces (pas le roi) vers n'importe quelle case vide, sans tenir compte des obstacles.", {}),
  e("imune", "Imune", "defense", "Rend une de vos pièces (pas le roi) invulnérable pendant le prochain tour adverse.", {}),
  e("rollback", "Rollback", "mobility", "Ramène une de vos pièces (pas le roi) sur la case d'où elle vient, si elle est libre.", {}),
  e("clone", "Clone", "create", "Copie une de vos pièces (pas le roi) sur une case vide adjacente.", {}),
  e("morph", "Morph", "create", "Transforme une pièce en une autre : un tour si elle est ennemie, deux sinon."),
  e("canceller", "Canceller", "control", "Annule l'effet d'une compétence ennemie utilisée au tour précédent."),
  e("tornado", "Tornado", "control", "Fait tourner toutes les pièces de l'échiquier dans un sens, sauf les rois. Un pion n'est jamais promu."),
  e("invisibility", "Invisibility", "defense", "Rend une de vos pièces invisible pendant deux tours."),
  e("freeze", "Freeze", "control", "Empêche une pièce ennemie (pas le roi) de se déplacer pendant deux de ses tours.", {}),
  e("terminator", "Terminator", "attack", "Copie une pièce ennemie aléatoire, avec ses capacités, de votre côté, pour un tour."),
  e("destiny_swapper", "Destiny Swapper", "mobility", "Échange les positions de deux de vos pièces.", {}),
  e("trap", "Trap Card", "attack", "Pose un piège sur une case vide : la première pièce ennemie qui y passe est immobilisée deux tours."),
  e("bench", "The Bench", "mobility", "Met une pièce sur le banc pendant un tour ; elle revient sur la case libre la plus proche."),
  e("forcefield", "Force Field", "defense", "Si la pièce protégée est mangée, l'attaquant est repoussé de deux cases au maximum."),
  e("transposition", "Transposition", "mobility", "Échange la position de deux pièces, sans provoquer d'échec."),
  e("queensac", "Queen Sacrifice", "attack", "En cas de mat, échange la reine et le roi ; la reine meurt à la place du roi."),
  e("temporal", "Temporal Distortion", "mobility", "Une pièce alliée remonte le temps et revient sur la case d'où elle vient (capture possible), à la place de votre coup."),
  e("geomancy", "Geomancy", "control", "Déplace des cases pour dresser des murs qui bloquent l'adversaire pendant trois tours."),
  e("celestial", "Celestial Intervention", "defense", "Une fois par partie, évite la capture d'une pièce alliée et la replace à son départ."),
  e("godhelp", "God Help", "create", "Une pièce aléatoire, aux capacités aléatoires, apparaît pendant trois tours."),
  // Compétences uniques
  e("remover", "Remover", "attack", "Retire un pion adverse de l'échiquier, sauf si cela provoque un mat.", { unique: true }),
  e("wall", "Wall", "create", "Ramène des pions à la vie sous forme de mur de protection, jouables dès le tour suivant.", { unique: true }),
  e("mirage", "Mirage", "create", "Simule une pièce sur l'échiquier (pas le roi) ; elle disparaît si elle est prise.", { unique: true }),
  e("evolve", "Evolve", "create", "Fait évoluer une pièce comme si elle avait franchi une case d'évolution adverse.", { unique: true }),
  e("switch", "Switch Sides", "attack", "Fait changer de camp une pièce adverse hors de portée et de ligne de vue.", { unique: true }),
  e("mind", "Mind Reading", "control", "Donne le meilleur coup possible, trois fois par partie.", { unique: true }),
  e("control", "Mind Control", "control", "Prend le contrôle d'une pièce adverse pendant un tour complet (pas le roi).", { unique: true }),
];

export const CATALOG_BY_ID: Record<string, CatalogEntry> = Object.fromEntries(CATALOG.map((c) => [c.id, c]));

/** Fiche d'une compétence ; une compétence inconnue du client reçoit une fiche neutre. */
export function skillEntry(id: string): CatalogEntry {
  if (isForgedId(id)) {
    const def = forgedDef(id);
    if (def) {
      return {
        id,
        name: def.name,
        family: def.family,
        unique: def.unique,
        description: def.description,
        implemented: true,
        rarity: def.rarity,
      };
    }
    // Pas encore reçue (la requête est partie) : une fiche d'attente.
    return { id, name: "Pouvoir forgé", family: "control", unique: false, description: "", implemented: true };
  }
  return (
    CATALOG_BY_ID[id] ?? {
      id: id as SkillId,
      name: id.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase()),
      family: "control",
      unique: false,
      description: "",
      implemented: false,
    }
  );
}

export function familyVar(family: Family): string {
  return `var(--fam-${family})`;
}

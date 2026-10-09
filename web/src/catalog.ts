import { t } from "./i18n";
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

export const FAMILIES: Family[] = ["attack", "defense", "mobility", "control", "create"];

/** Libellé de chaque famille, traduit à chaque lecture (propriétés dynamiques). */
export const FAMILY_LABEL: Record<Family, string> = Object.defineProperties(
  {} as Record<Family, string>,
  Object.fromEntries(FAMILIES.map((f) => [f, { enumerable: true, get: () => t(`catalog.family_${f}`) }])),
);

/** Une fiche des 27 compétences écrites à la main. */
export type BuiltinEntry = CatalogEntry & { id: CatalogId };

// `name` et `description` sont traduits à chaque lecture (clés `catalog.name_<id>` / `catalog.desc_<id>`).
const e = (id: CatalogId, family: Family, opts: { unique?: boolean; implemented?: boolean } = {}): BuiltinEntry => ({
  id,
  get name() {
    return t(`catalog.name_${id}`);
  },
  family,
  unique: opts.unique ?? false,
  get description() {
    return t(`catalog.desc_${id}`);
  },
  implemented: opts.implemented ?? true,
});

export const CATALOG: readonly BuiltinEntry[] = [
  // Compétences classiques
  e("teleportation", "mobility", {}),
  e("imune", "defense", {}),
  e("rollback", "mobility", {}),
  e("clone", "create", {}),
  e("morph", "create"),
  e("canceller", "control"),
  e("tornado", "control"),
  e("invisibility", "defense"),
  e("freeze", "control", {}),
  e("terminator", "attack"),
  e("destiny_swapper", "mobility", {}),
  e("trap", "attack"),
  e("bench", "mobility"),
  e("forcefield", "defense"),
  e("transposition", "mobility"),
  e("queensac", "attack"),
  e("temporal", "mobility"),
  e("geomancy", "control"),
  e("celestial", "defense"),
  e("godhelp", "create"),
  // Compétences uniques
  e("remover", "attack", { unique: true }),
  e("wall", "create", { unique: true }),
  e("mirage", "create", { unique: true }),
  e("evolve", "create", { unique: true }),
  e("switch", "attack", { unique: true }),
  e("mind", "control", { unique: true }),
  e("control", "control", { unique: true }),
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
    return { id, name: t("catalog.forged_pending"), family: "control", unique: false, description: "", implemented: true };
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

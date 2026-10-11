export type NodePoint = readonly [x: number, y: number];

export interface MapLayout {
  width: number;
  height: number;
  /** Un point par niveau, dans l'ordre : le sentier relie les niveaux un à un, le boss en dernier. */
  nodes: readonly NodePoint[];
}

/** Sentier de la carte d'un chapitre, en unités de la boîte `width` × `height`. */
export const MAP_LAYOUTS: { desktop: MapLayout; phone: MapLayout } = {
  desktop: { width: 800, height: 440, nodes: [[80, 340], [210, 290], [340, 330], [470, 270], [360, 170], [500, 100], [680, 120]] },
  phone: { width: 360, height: 400, nodes: [[70, 350], [201, 310], [292, 240], [161, 200], [70, 130], [191, 80], [297, 90]] },
};

export function percent(value: number, total: number): string {
  return `${(value / total) * 100}%`;
}

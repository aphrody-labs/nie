/**
 * Clé de catégorie d'objet, en `snake_case`.
 *
 * `inagle_items.category` arrive sous deux graphies (`Shoes`, `NamePlate`,
 * `KizunaLink`, `CraftObj`… et `important`, `special_tactics`) alors que toutes les
 * tables de libellés et d'icônes du wiki, le filtre `?category=` et les tests
 * attendent du `snake_case`. On normalise à la lecture, dans les modules de données,
 * pour que les composants n'aient pas à compenser.
 */
export function normalizeItemCategory(
  category: string | null | undefined,
): string {
  if (!category) return "";
  return String(category)
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[\s-]+/g, "_")
    .toLowerCase();
}

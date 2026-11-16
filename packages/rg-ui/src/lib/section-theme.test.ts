import { expect, test } from "bun:test";

import { sectionTheme, sectionThemeSelonChoix } from "./themes";

test("sans choix, chaque section garde son thème par défaut", () => {
  expect(sectionThemeSelonChoix("wiki", false)).toBe(sectionTheme("wiki"));
  expect(sectionThemeSelonChoix("esport", false)).toBe("achillea");
  expect(sectionThemeSelonChoix("association", false)).toBeUndefined();
});

test("un thème choisi s'applique au wiki et à l'e-sport", () => {
  expect(sectionThemeSelonChoix("wiki", true)).toBeUndefined();
  expect(sectionThemeSelonChoix("esport", true)).toBeUndefined();
});

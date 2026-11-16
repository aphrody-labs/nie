import { describe, expect, test } from "bun:test";
import { Ui } from "../src/ui/ui";

describe("@rosegriffon/nie-ui exports", () => {
  test("Ui class is exported and defined", () => {
    expect(Ui).toBeDefined();
  });
});

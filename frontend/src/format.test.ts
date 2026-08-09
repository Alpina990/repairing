import { describe, expect, it } from "vitest";
import { formatDuration, initials } from "./format";

describe("formatDuration", () => {
  it("formats moderation durations", () => {
    expect(formatDuration(30)).toBe("30s");
    expect(formatDuration(3600)).toBe("1h");
    expect(formatDuration(86_400)).toBe("1d");
  });
});

describe("initials", () => {
  it("uses at most two words", () => {
    expect(initials("Alisher Karimov")).toBe("AK");
  });
});

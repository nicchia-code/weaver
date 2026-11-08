import { describe, expect, it } from "vitest";

import { fileChips } from "@/components/board/file-chips";
import { viewData } from "@/components/views/view-fixtures";

describe("the file chips of a Task", () => {
  it("names the fragment that holds the section a reference concerns, once, with the first reference that names it", () => {
    const chips = fileChips(["order/Paid", "order", "requirements/orders#order"], viewData());
    // `order/Paid` concerns no section in the fixture; `order` and the section reference name `checkout.md`.
    expect(chips).toEqual([{ key: "requirements:orders/checkout", label: "checkout.md", view: "requirements", ref: "order" }]);
  });

  it("names the Specs file for one field and tells the fields apart when there are several", () => {
    expect(fileChips(["specs#languages"], viewData())).toEqual([{ key: "specs:languages", label: "specs.yaml", view: "specs", ref: "specs#languages" }]);
    expect(fileChips(["specs#languages", "specs#notes"], viewData()).map((chip) => chip.label)).toEqual(["specs.yaml · languages", "specs.yaml · notes"]);
  });

  it("keeps the order of the references: fragments first, then the Specs fields", () => {
    const labels = fileChips(["specs#languages", "order"], viewData()).map((chip) => chip.label);
    expect(labels).toEqual(["checkout.md", "specs.yaml"]);
  });

  it("names no file for an Area, an unknown reference or no data", () => {
    expect(fileChips(["requirements/ui", "nothing"], viewData())).toEqual([]);
    expect(fileChips(["order"], null)).toEqual([]);
    expect(fileChips(["order"], undefined)).toEqual([]);
  });

  it("tells two fragments of one name by their Area", () => {
    const data = viewData();
    data.requirements[1]!.fragments[0] = { ...data.requirements[1]!.fragments[0]!, name: "checkout" };
    data.targets["requirements/ui#General"] = { relevant: ["requirements"], requirements: [{ area: "ui", section: "General" }], specs: [] };
    const labels = fileChips(["order", "requirements/ui#General"], data).map((chip) => chip.label);
    expect(labels).toEqual(["orders/checkout.md", "ui/checkout.md"]);
  });
});

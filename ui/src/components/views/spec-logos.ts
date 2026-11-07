import { useEffect, useState } from "react";

/** The logos of `simple-icons`, by the name they are looked up with (`logoKey`): the SVG path of each. */
export type LogoIndex = ReadonlyMap<string, string>;

interface Logo {
  title: string;
  slug: string;
  path: string;
}

/** A name compared whole, ignoring case, spaces and punctuation: `Node.js` and `node js` are the same. */
export function logoKey(name: string): string {
  return name.toLowerCase().replace(/[^\p{L}\p{N}]/gu, "");
}

const isLogo = (value: unknown): value is Logo =>
  typeof value === "object" && value !== null && "title" in value && "slug" in value && "path" in value;

/** Every logo under its title and under its slug; a title wins over another logo's slug. */
function indexOf(collection: Record<string, unknown>): LogoIndex {
  const logos = Object.values(collection).filter(isLogo);
  const index = new Map<string, string>();
  for (const { slug, path } of logos) index.set(logoKey(slug), path);
  for (const { title, path } of logos) index.set(logoKey(title), path);
  return index;
}

let loaded: LogoIndex | null = null;
let loading: Promise<LogoIndex | null> | undefined;

/**
 * The whole collection is bundled in the app as a chunk of its own and loaded by one `import()`, the
 * first time a Specs view opens in the window; it is never fetched from the network. A collection
 * that cannot be loaded resolves to `null`, with no notice: the field icons stay.
 */
export function loadLogos(): Promise<LogoIndex | null> {
  // Dynamic on purpose: the collection is large and the spec loads it at the first Specs view, not at start.
  loading ??= import("simple-icons").then(
    (collection) => (loaded = indexOf(collection)),
    () => {
      loading = undefined;
      return null;
    },
  );
  return loading;
}

/** The logos once the collection is there, `null` until then (and for good when it cannot be loaded). Never waits. */
export function useLogos(): LogoIndex | null {
  const [logos, setLogos] = useState<LogoIndex | null>(loaded);
  useEffect(() => {
    let current = true;
    void loadLogos().then((index) => {
      if (current && index) setLogos(index);
    });
    return () => {
      current = false;
    };
  }, []);
  return logos;
}

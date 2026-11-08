/**
 * Weaver's logo (`requirements/ui#window`): a cream rounded square with a Line ring, crossed by a
 * horizontal Sky thread and by four diagonal threads, Peach, Lavender, Rose and Sage, that weave a
 * "W". It is the drawing of `src-tauri/icons/source.svg`, the master of the app icon (`logo.test.tsx`
 * keeps the two equal), shown at `size` px. Decorative: hidden from assistive technologies, so the
 * name it accompanies must be written next to it.
 */
export function Logo({ size = 30, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 30 30" aria-hidden="true" focusable="false" className={className}>
      <rect x="0.5" y="0.5" width="29" height="29" rx="8.5" fill="#FBF6EE" stroke="#E7E5DF" />
      <path d="M5.5 15h19" stroke="#96BBE0" strokeWidth="2" strokeLinecap="round" opacity="0.8" />
      <path d="M7 8.5l4 13" stroke="#EDB08F" strokeWidth="3.2" strokeLinecap="round" />
      <path d="M11 21.5l4-11" stroke="#A996D6" strokeWidth="3.2" strokeLinecap="round" />
      <path d="M15 10.5l4 11" stroke="#E3A3B5" strokeWidth="3.2" strokeLinecap="round" />
      <path d="M19 21.5l4-13" stroke="#9DC09A" strokeWidth="3.2" strokeLinecap="round" />
    </svg>
  );
}

import { HighlightStyle, syntaxHighlighting } from "@codemirror/language";
import type { Extension } from "@codemirror/state";
import { EditorView } from "@codemirror/view";
import { tags } from "@lezer/highlight";

import type { ResolvedTheme } from "@/components/theme-provider";

/**
 * The colours of the editor in each theme. Every text colour has at least 4.5:1 against the editor
 * background and against the selection (`requirements/quality#General`, WCAG 2.2 AA); a test checks
 * the pairs. The background is Surface and the text Ink; the syntax colours are the inks of the five
 * threads (`requirements/ui#General`), the lighter ones on dark, so they match the rest of the UI.
 */
export interface EditorPalette {
  background: string;
  selection: string;
  foreground: string;
  punctuation: string;
  comment: string;
  keyword: string;
  string: string;
  number: string;
  definition: string;
  type: string;
  link: string;
  invalid: string;
}

export const PALETTES: Record<ResolvedTheme, EditorPalette> = {
  light: {
    background: "#ffffff",
    selection: "#d6e5f5",
    foreground: "#18181b",
    punctuation: "#3f3f46",
    comment: "#52525b",
    keyword: "#5b4a99",
    string: "#3f6b3c",
    number: "#2e5f8f",
    definition: "#2e5f8f",
    type: "#9a4a22",
    link: "#2e5f8f",
    invalid: "#9b3b57",
  },
  dark: {
    background: "#1f1f1d",
    selection: "#223a52",
    foreground: "#f4f4f2",
    punctuation: "#b6b6b0",
    comment: "#a8a8a2",
    keyword: "#c0b2e8",
    string: "#a8d2a5",
    number: "#9dc4ea",
    definition: "#9dc4ea",
    type: "#f2b998",
    link: "#9dc4ea",
    invalid: "#eeaebf",
  },
};

function highlightStyle(palette: EditorPalette): HighlightStyle {
  return HighlightStyle.define([
    { tag: [tags.keyword, tags.operatorKeyword, tags.modifier, tags.controlKeyword], color: palette.keyword },
    { tag: [tags.string, tags.special(tags.string), tags.regexp, tags.character], color: palette.string },
    { tag: [tags.number, tags.bool, tags.null, tags.atom, tags.literal], color: palette.number },
    { tag: [tags.comment, tags.lineComment, tags.blockComment, tags.docComment], color: palette.comment, fontStyle: "italic" },
    {
      tag: [tags.function(tags.variableName), tags.function(tags.propertyName), tags.definition(tags.variableName), tags.definition(tags.propertyName), tags.labelName],
      color: palette.definition,
    },
    { tag: [tags.typeName, tags.className, tags.namespace, tags.tagName, tags.standard(tags.tagName)], color: palette.type },
    { tag: [tags.propertyName, tags.attributeName, tags.variableName], color: palette.foreground },
    { tag: [tags.operator, tags.punctuation, tags.separator, tags.bracket, tags.meta], color: palette.punctuation },
    { tag: [tags.heading, tags.strong], fontWeight: "bold", color: palette.foreground },
    { tag: tags.emphasis, fontStyle: "italic" },
    { tag: tags.strikethrough, textDecoration: "line-through" },
    { tag: [tags.link, tags.url], color: palette.link, textDecoration: "underline" },
    { tag: tags.invalid, color: palette.invalid },
  ]);
}

/** The editor's chrome and syntax colours for a resolved theme. */
export function editorTheme(theme: ResolvedTheme): Extension {
  const palette = PALETTES[theme];
  return [
    EditorView.theme(
      {
        "&": { color: palette.foreground, backgroundColor: palette.background, height: "100%" },
        ".cm-scroller": {
          fontFamily: '"Geist Mono Variable", ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
          fontSize: "13px",
          lineHeight: "1.5",
        },
        ".cm-content": { caretColor: palette.foreground },
        "&.cm-focused .cm-cursor": { borderLeftColor: palette.foreground },
        "&.cm-focused": { outline: "none" },
        "&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, .cm-content ::selection": {
          backgroundColor: palette.selection,
        },
        ".cm-gutters": { backgroundColor: palette.background, color: palette.comment, border: "none" },
        ".cm-activeLineGutter": { backgroundColor: "transparent", color: palette.foreground },
        ".cm-matchingBracket": { backgroundColor: palette.selection, outline: `1px solid ${palette.punctuation}` },
        ".cm-panels": { backgroundColor: palette.background, color: palette.foreground },
        ".cm-panels.cm-panels-top": { borderBottom: `1px solid ${palette.comment}` },
        ".cm-textfield, .cm-button": { color: palette.foreground, backgroundColor: palette.background },
        ".cm-searchMatch": { backgroundColor: palette.selection, outline: `1px solid ${palette.keyword}` },
      },
      { dark: theme === "dark" },
    ),
    syntaxHighlighting(highlightStyle(palette)),
  ];
}

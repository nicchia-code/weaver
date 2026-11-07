/** Nothing left after removing whitespace; a request of images only is not empty (its text has the links). */
export function isEmptyRequest(text: string): boolean {
  return text.trim() === "";
}

/** The local time as `YYYY-MM-DDTHH:MM:SS`, the creation time the backend names a request after. */
export function localTimeOf(date: Date): string {
  const two = (n: number) => String(n).padStart(2, "0");
  const day = `${date.getFullYear()}-${two(date.getMonth() + 1)}-${two(date.getDate())}`;
  return `${day}T${two(date.getHours())}:${two(date.getMinutes())}:${two(date.getSeconds())}`;
}

/**
 * The text as the file keeps it: line endings LF and no more than the final newline the file adds.
 * A disk text and a typed one are the same text when this is equal.
 */
export function sameText(a: string, b: string): boolean {
  const key = (text: string) => text.replace(/\r\n/g, "\n").replace(/\n$/, "");
  return key(a) === key(b);
}

/** `text` with `insert` in place of the selection `start..end`, and where the cursor goes after it. */
export function insertAt(text: string, start: number, end: number, insert: string): { text: string; cursor: number } {
  return { text: text.slice(0, start) + insert + text.slice(end), cursor: start + insert.length };
}

export const IMAGE_TYPES = ["image/png", "image/jpeg", "image/gif", "image/webp"] as const;
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "gif", "webp"] as const;
/** The largest image a request takes. */
export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
export const IMAGE_REFUSED = "Only PNG, JPEG, GIF or WebP images up to 10 MB.";

/** An image of an accepted type and size (`size` of the file, or `null` when it is not known). */
export function acceptableImage(type: string, size: number | null): boolean {
  return (IMAGE_TYPES as readonly string[]).includes(type) && (size === null || size <= MAX_IMAGE_BYTES);
}

/** A path with the extension of an accepted image. */
export function hasImageExtension(path: string): boolean {
  const extension = /\.([A-Za-z0-9]+)$/.exec(path)?.[1]?.toLowerCase();
  return extension !== undefined && (IMAGE_EXTENSIONS as readonly string[]).includes(extension);
}

/** The links of the text that point into the images folder `<id>.images/` of the request. */
export function ownImageLinks(text: string, requestId: string): string[] {
  const links = new Set<string>();
  for (const match of text.matchAll(/!\[[^\]]*\]\(([^)\s]+)\)/g)) {
    const link = match[1];
    if (link?.startsWith(`${requestId}.images/`)) links.add(link);
  }
  return [...links];
}

/** Base64 of bytes (what the backend takes for a pasted image). */
export function toBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunk = 0x8000;
  for (let i = 0; i < bytes.length; i += chunk) binary += String.fromCharCode(...bytes.subarray(i, i + chunk));
  return btoa(binary);
}

/** The toast when a request being written was deleted from outside. */
export const TOAST_DELETED = "The request was deleted outside Weaver.";

// A list's prose is Markdown written by whoever wrote the list: rendered, then
// sanitised, since the page can call commands that write files.

import { marked } from "marked";
import DOMPurify from "dompurify";

marked.setOptions({ gfm: true, breaks: false });

// Links open in the browser, never in the app's own window.
DOMPurify.addHook("afterSanitizeAttributes", (node) => {
  if (node.tagName === "A") {
    node.setAttribute("target", "_blank");
    node.setAttribute("rel", "noreferrer");
  }
});

export function markdown(text: string | undefined | null): string {
  if (!text) return "";
  return DOMPurify.sanitize(marked.parse(text, { async: false }) as string);
}

// One line of Markdown (a summary): emphasis and links, no paragraph.
export function inline(text: string | undefined | null): string {
  if (!text) return "";
  return DOMPurify.sanitize(marked.parseInline(text, { async: false }) as string);
}

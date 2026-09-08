export const TEXT_PAGE_SIZE = 32768;

// Adjacent pages share a boundary before the whole surrogate pair, never half an emoji.
export function textPage(text: string, page: number): string {
  let start = page * TEXT_PAGE_SIZE;
  let end = (page + 1) * TEXT_PAGE_SIZE;
  const low = (index: number) =>
    text.charCodeAt(index) >= 0xdc00 && text.charCodeAt(index) <= 0xdfff;
  if (start > 0 && low(start)) start--;
  if (end < text.length && low(end)) end--;
  return text.slice(start, end);
}

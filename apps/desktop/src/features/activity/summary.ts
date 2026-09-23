/** Session activity keeps a brief first nonempty line, not another copy of the full text. */
export function activitySummary(text: string) {
  const line =
    text
      .split(/\r?\n/)
      .find((part) => part.trim())
      ?.trim() ?? '';
  const chars = Array.from(line.replace(/\s+/g, ' '));
  return chars.length > 160 ? chars.slice(0, 160).join('') + '…' : chars.join('');
}

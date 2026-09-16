/** A small postal mark ties the receipt to the mailbox without carrying status. */
export function Postmark() {
  return (
    <svg className="postmark" viewBox="0 0 100 62" fill="none" aria-hidden="true">
      <circle cx="34" cy="31" r="25" />
      <circle cx="34" cy="31" r="20" />
      <path d="M54 21c8-7 13 7 22 0s13 7 20 0M57 31c8-7 13 7 22 0s10 5 17 0M54 41c8-7 13 7 22 0s13 7 20 0" />
      <text x="34" y="33" textAnchor="middle">
        NOOBOARD
      </text>
      <path d="M29 40h10M30 22h8" />
    </svg>
  );
}

import { useEffect } from 'react';

/** Writes how far the landing page has been scrolled (0 to 1) into `--scroll` on the page, for the thin progress line at the top. */
export function useScrollProgress() {
  useEffect(() => {
    const root = document.querySelector<HTMLElement>('.landing');
    if (!root) return;
    const update = () => root.style.setProperty('--scroll', String(Math.min(1, root.scrollTop / Math.max(1, root.scrollHeight - root.clientHeight))));
    update();
    root.addEventListener('scroll', update, { passive: true });
    return () => root.removeEventListener('scroll', update);
  }, []);
}

/**
 * A soft light that follows the pointer across cards: it writes the pointer's position, relative to the card, into `--mx` and `--my`, and the stylesheet draws
 * the glow from them. One listener on the page, and none at all for a visitor who asked for less motion or has no hover.
 */
export function useSpotlight(selector = '.tool-categories article, .project-card, .stat') {
  useEffect(() => {
    if (matchMedia('(prefers-reduced-motion: reduce)').matches || !matchMedia('(hover: hover)').matches) return;
    const root = document.querySelector<HTMLElement>('.landing');
    if (!root) return;
    const move = (event: PointerEvent) => {
      const card = (event.target as Element | null)?.closest<HTMLElement>(selector);
      if (!card) return;
      const box = card.getBoundingClientRect();
      card.style.setProperty('--mx', `${event.clientX - box.left}px`);
      card.style.setProperty('--my', `${event.clientY - box.top}px`);
    };
    root.addEventListener('pointermove', move, { passive: true });
    return () => root.removeEventListener('pointermove', move);
  }, [selector]);
}

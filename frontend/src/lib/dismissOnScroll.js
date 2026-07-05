/**
 * dismissOnScroll — close a native popover when the user scrolls behind it.
 *
 * Our note popovers are anchored to a fixed viewport position (a phantom
 * element at a selection/badge rect), so the moment anything scrolls that
 * anchor is stale and the popover floats free of its target. Rather than chase
 * it, dismiss it.
 *
 * Catching the scroll is the tricky part: the text editor grows and the article
 * scrolls inside a DS layout component's OWN shadow root, where a plain `scroll`
 * event (composed: false) never reaches the document. So we also listen for
 * `wheel` and `touchmove`, which ARE composed and cross shadow boundaries. A
 * scroll that starts inside the popover itself is ignored so its own content
 * (a long note list, the form) stays scrollable.
 *
 * @param {HTMLElement} popEl  The nldd-popover element (native popover API).
 * @returns {() => void}       Cleanup that removes the listeners.
 */
export function dismissPopoverOnScroll(popEl) {
  const onScroll = (event) => {
    // Only background scrolls dismiss; scrolling within the popover doesn't.
    if (event.composedPath?.().includes(popEl)) return;
    popEl?.hidePopover?.();
  };
  const opts = { capture: true, passive: true };
  window.addEventListener('scroll', onScroll, opts);
  window.addEventListener('wheel', onScroll, opts);
  window.addEventListener('touchmove', onScroll, opts);
  return () => {
    window.removeEventListener('scroll', onScroll, opts);
    window.removeEventListener('wheel', onScroll, opts);
    window.removeEventListener('touchmove', onScroll, opts);
  };
}

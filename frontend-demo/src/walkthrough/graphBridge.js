/**
 * Where the walkthrough meets the graph's camera.
 *
 * Panning and zooming the graph (vue-flow) happens through dragging and the
 * mouse wheel, which the action log does not see: there is no element that
 * was clicked. So the graph reports where it looks, and the replay tells it
 * where to look. Both as the point in the middle of the pane, in graph
 * coordinates, plus the zoom: a position in pixels would land elsewhere on a
 * screen of another size.
 *
 * GraafView fills `graphView.api` while it is mounted; the recorder sets
 * `graphView.onView` while a take runs.
 */

export const graphView = {
  /** `{ setCenter, getViewport, dimensions }` from vue-flow, or null. */
  api: null,
  /** Called with `{ cx, cy, zoom }` whenever the view moves. */
  onView: null,
};

/** The middle of the pane in graph coordinates, from a vue-flow viewport. */
export function centerOf(viewport, dimensions) {
  const { x = 0, y = 0, zoom = 1 } = viewport ?? {};
  const w = dimensions?.width ?? 0;
  const h = dimensions?.height ?? 0;
  const round = (n) => Math.round(n * 10) / 10;
  return { cx: round((w / 2 - x) / zoom), cy: round((h / 2 - y) / zoom), zoom: Math.round(zoom * 1000) / 1000 };
}

/** Tell the graph to look at `view`, over `duration` ms. False when no graph is mounted. */
export function showView(view, duration = 0) {
  const api = graphView.api;
  if (!api?.setCenter) return false;
  api.setCenter(view.cx, view.cy, { zoom: view.zoom, duration });
  return true;
}

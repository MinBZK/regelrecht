/** Branch/fork colors — drives node fill color */
export const branchColors = {
  main: 'var(--color-branch-main)',
  develop: 'var(--color-branch-develop)',
  wetsvoorstel: 'var(--color-branch-wetsvoorstel)',
  internal: 'var(--color-branch-internal)',
  advisory: 'var(--color-branch-advisory)',
};

/** Legacy type→color map for advanced/Woo views */
export const typeColors = {
  commit: 'var(--color-branch-main)',
  branch: 'var(--color-branch-feature)',
  'ci-check': 'var(--color-ci)',
  review: 'var(--color-review)',
  merge: 'var(--color-branch-main)',
  deploy: 'var(--color-deploy)',
  release: 'var(--color-release)',
};

/** Legend entries — one per branch type */
export const legend = [
  { label: 'Corpus Juris (main)', color: 'var(--color-branch-main)' },
  { label: 'Wetgevingskalender (develop)', color: 'var(--color-branch-develop)' },
  { label: 'Wetsvoorstel (fork)', color: 'var(--color-branch-wetsvoorstel)' },
  { label: 'Interne afstemming', color: 'var(--color-branch-internal)' },
  { label: 'Advies / validatie (fork)', color: 'var(--color-branch-advisory)' },
];

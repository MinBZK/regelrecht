/** Color for each node type */
export const typeColors = {
  commit: 'var(--color-branch-main)',
  branch: 'var(--color-branch-feature)',
  'ci-check': 'var(--color-ci)',
  review: 'var(--color-review)',
  merge: 'var(--color-branch-main)',
  deploy: 'var(--color-deploy)',
  release: 'var(--color-release)',
};

/** Icon/shape for each node type */
export const typeShapes = {
  commit: 'circle',
  branch: 'circle',
  'ci-check': 'diamond',
  review: 'square',
  merge: 'circle-double',
  deploy: 'triangle',
  release: 'star',
};

/** Legend entries */
export const legend = [
  { label: 'Commit', shape: 'circle', color: 'var(--color-branch-main)' },
  { label: 'Branch', shape: 'circle', color: 'var(--color-branch-feature)' },
  { label: 'CI Check', shape: 'diamond', color: 'var(--color-ci)' },
  { label: 'Review', shape: 'square', color: 'var(--color-review)' },
  { label: 'Deploy', shape: 'triangle', color: 'var(--color-deploy)' },
  { label: 'Release', shape: 'star', color: 'var(--color-release)' },
];

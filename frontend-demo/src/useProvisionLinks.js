import { provisionLabel, provisionTarget } from './data/chronolex.js';
import { useLocalePath } from './i18n/useLocalePath.js';

/**
 * Een bepaling (`<regelwerk>#<artikel>[ lid n]`) als link naar haar artikel
 * in Regelwerken: haar naam zoals een mens haar leest (`label`), of de demo
 * haar kan openen (`linkable`: alleen een wet uit het corpus) en het openen
 * zelf (`openProvision`; een lid opent zijn artikel). `corpus` is de ref uit
 * de store.
 */
export function useProvisionLinks(corpus) {
  const { goTo } = useLocalePath();
  return {
    label: (provision) => provisionLabel(corpus.value, provision),
    linkable: (provision) => !!provisionTarget(corpus.value, provision),
    openProvision(provision) {
      const target = provisionTarget(corpus.value, provision);
      if (!target) return;
      goTo('wetten', { lawId: target.lawId }, target.article ? { artikel: target.article } : undefined);
    },
  };
}

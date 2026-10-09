import { useRouter } from 'vue-router';
import { provisionLabel, provisionTarget } from './data/chronolex.js';
import { useLocalePath } from './i18n/useLocalePath.js';

/**
 * Een bepaling (`<regelwerk>#<artikel>[ lid n]`) als link naar haar artikel
 * in Regelwerken: haar naam zoals een mens haar leest (`label`), of de demo
 * haar kan openen (`linkable`: alleen een wet uit het corpus), het adres
 * (`href`, voor een echte link) en het openen zelf (`openProvision`; een lid
 * opent zijn artikel). Een artikel dat de wet in de demo niet bevat, opent op
 * wetten.overheid.nl, in een nieuw tabblad (`external`), in de tekst die op
 * de peildatum gold. `corpus` is de ref uit de store, `referenceDate` een
 * functie die de peildatum geeft (de versie van de wet die dan geldt).
 */
export function useProvisionLinks(corpus, referenceDate = () => null) {
  const router = useRouter();
  const { localePath, goTo } = useLocalePath();
  const target = (provision) => provisionTarget(corpus.value, provision, referenceDate());
  return {
    label: (provision) => provisionLabel(corpus.value, provision),
    linkable: (provision) => !!target(provision),
    external: (provision) => !!target(provision)?.external,
    href(provision) {
      const to = target(provision);
      if (!to) return undefined;
      if (to.external) return to.external;
      const path = localePath('wetten', { lawId: to.lawId });
      return to.article ? router.resolve({ path, query: { artikel: to.article } }).fullPath : path;
    },
    openProvision(provision) {
      const to = target(provision);
      if (!to) return;
      if (to.external) {
        window.open(to.external, '_blank', 'noopener');
        return;
      }
      goTo('wetten', { lawId: to.lawId }, to.article ? { artikel: to.article } : undefined);
    },
  };
}

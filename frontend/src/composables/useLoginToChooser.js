import { useRoute, useRouter } from 'vue-router';
import { useAuth } from './useAuth.js';

// Shared "log in, then land on the traject chooser" redirect. Used by TrajectMenu
// and MobileTrajectSheet so the return target (scoped to library vs editor, and
// carrying the open law/article) stays in sync between the two entry points.
export function useLoginToChooser() {
  const route = useRoute();
  const router = useRouter();
  const { login } = useAuth();

  return function loginToChooser() {
    const inLibrary = route.name === 'library' || route.name === 'library-traject';
    const target = router.resolve({
      name: 'trajecten',
      query: {
        sectie: inLibrary ? 'library' : 'editor',
        law: route.params.lawId || undefined,
        article: route.params.articleNumber || undefined,
      },
    });
    login(target.fullPath);
  };
}

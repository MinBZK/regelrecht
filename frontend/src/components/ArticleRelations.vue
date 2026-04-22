<script setup>
/**
 * ArticleRelations — renders cross-law relations for a single article.
 *
 * Outbound (from the article's own machine_readable):
 *   - implements: "vult open term X in [wet] art Y"
 *   - overrides:  "overschrijft output Z van [wet] art Y"
 *   - open_terms: "gedelegeerd aan [delegation_type]; wordt ingevuld door [wet]"
 *   - enables:    "maakt mogelijk: [regulatory_layer] voor [subject]"
 *
 * Inbound (reverse-lookups across the loaded corpus):
 *   - implementors: wie vult de open_terms van dit artikel in
 *   - overriders:   wie overschrijft outputs van dit artikel
 *   - users:        wie sourcet outputs van dit artikel
 *
 * The reverse-lookup scans the corpus once on mount (or when lawId/article
 * change). Results are cached per (lawId, article) for the session.
 */
import { computed, ref, watchEffect } from 'vue';
import { useRouter } from 'vue-router';
import {
  extractImplements,
  extractOverrides,
  extractOpenTerms,
  extractEnables,
  discoverImplementors,
  discoverOverriders,
  discoverUsers,
} from '../composables/useDependencies.js';

const props = defineProps({
  /** Parsed law object (top-level, with `$id` + `articles`). */
  law: { type: Object, default: null },
  /** The article object currently shown in the editor. */
  article: { type: Object, default: null },
});

const router = useRouter();

const articleNum = computed(() =>
  props.article ? String(props.article.number) : null,
);
const lawId = computed(() => props.law?.$id ?? null);

// --- Outbound (sync, no corpus scan) ---
const implementsOut = computed(() => {
  if (!props.law || !articleNum.value) return [];
  return extractImplements(props.law).filter((i) => i.article === articleNum.value);
});
const overridesOut = computed(() => {
  if (!props.law || !articleNum.value) return [];
  return extractOverrides(props.law).filter((o) => o.article === articleNum.value);
});
const openTermsOut = computed(() => {
  if (!props.law || !articleNum.value) return [];
  return extractOpenTerms(props.law).filter((t) => t.article === articleNum.value);
});
const enablesOut = computed(() => {
  if (!props.law || !articleNum.value) return [];
  return extractEnables(props.law).filter((e) => e.article === articleNum.value);
});

// --- Inbound (async reverse-lookups) ---
const implementors = ref([]);
const overriders = ref([]);
const users = ref([]);
const reverseLoading = ref(false);

// Session cache keyed by `${lawId}:${article}` to avoid re-scanning on re-mount
const reverseCache = new Map();

async function fetchLawYaml(targetLawId) {
  const res = await fetch(`/api/corpus/laws/${encodeURIComponent(targetLawId)}`);
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const json = await res.json();
  return json.yaml ?? json.content ?? json;
}

async function loadReverse() {
  implementors.value = [];
  overriders.value = [];
  users.value = [];
  if (!lawId.value || !articleNum.value) return;

  const cacheKey = `${lawId.value}:${articleNum.value}`;
  if (reverseCache.has(cacheKey)) {
    const cached = reverseCache.get(cacheKey);
    implementors.value = cached.implementors;
    overriders.value = cached.overriders;
    users.value = cached.users;
    return;
  }

  reverseLoading.value = true;
  try {
    const res = await fetch('/api/corpus/laws?limit=1000');
    if (!res.ok) return;
    const allLaws = await res.json();

    const outputsInArticle = (props.article.machine_readable?.execution?.output || [])
      .map((o) => o.name);

    const [impls, ovs, allUsers] = await Promise.all([
      discoverImplementors(lawId.value, allLaws, fetchLawYaml, articleNum.value),
      discoverOverriders(lawId.value, allLaws, fetchLawYaml, articleNum.value),
      discoverUsers(lawId.value, allLaws, fetchLawYaml),
    ]);

    // Filter users to outputs that actually live in this article
    const filteredUsers = allUsers.filter((u) =>
      !u.target_output || outputsInArticle.includes(u.target_output),
    );

    reverseCache.set(cacheKey, {
      implementors: impls,
      overriders: ovs,
      users: filteredUsers,
    });
    implementors.value = impls;
    overriders.value = ovs;
    users.value = filteredUsers;
  } finally {
    reverseLoading.value = false;
  }
}

watchEffect(() => {
  if (lawId.value && articleNum.value) loadReverse();
});

function goTo(targetLawId, articleNumber) {
  const path = articleNumber
    ? `/editor/${encodeURIComponent(targetLawId)}/${encodeURIComponent(articleNumber)}`
    : `/editor/${encodeURIComponent(targetLawId)}`;
  router.push(path);
}

const hasAny = computed(() =>
  implementsOut.value.length ||
  overridesOut.value.length ||
  openTermsOut.value.length ||
  enablesOut.value.length ||
  implementors.value.length ||
  overriders.value.length ||
  users.value.length,
);
</script>

<template>
  <ndd-simple-section v-if="hasAny || reverseLoading" data-testid="article-relations">
    <ndd-title size="5"><h5>Relaties</h5></ndd-title>
    <ndd-spacer size="8"></ndd-spacer>

    <ndd-list variant="box">
      <!-- Outbound: implements -->
      <ndd-list-item v-for="item in implementsOut" :key="`impl-${item.target_law}-${item.target_article}-${item.open_term}`" size="md">
        <ndd-text-cell
          :text="`Vult open term '${item.open_term || '—'}'`"
          :supporting-text="item.gelet_op || ''"
        ></ndd-text-cell>
        <ndd-cell>
          <ndd-button
            size="md"
            :text="`${item.target_law}${item.target_article ? ' art ' + item.target_article : ''}`"
            @click="goTo(item.target_law, item.target_article)"
          ></ndd-button>
        </ndd-cell>
      </ndd-list-item>

      <!-- Outbound: overrides -->
      <ndd-list-item v-for="item in overridesOut" :key="`ov-${item.target_law}-${item.target_article}-${item.target_output}`" size="md">
        <ndd-text-cell :text="`Overschrijft output '${item.target_output}'`"></ndd-text-cell>
        <ndd-cell>
          <ndd-button
            size="md"
            :text="`${item.target_law}${item.target_article ? ' art ' + item.target_article : ''}`"
            @click="goTo(item.target_law, item.target_article)"
          ></ndd-button>
        </ndd-cell>
      </ndd-list-item>

      <!-- Outbound: open_terms -->
      <ndd-list-item v-for="term in openTermsOut" :key="`ot-${term.id}`" size="md">
        <ndd-text-cell
          :text="`Open term: ${term.id}`"
          :supporting-text="`Gedelegeerd aan ${term.delegated_to || '?'} (${term.delegation_type || '?'})${term.legal_basis ? ' — ' + term.legal_basis : ''}`"
        ></ndd-text-cell>
      </ndd-list-item>

      <!-- Outbound: enables -->
      <ndd-list-item v-for="(en, i) in enablesOut" :key="`en-${i}`" size="md">
        <ndd-text-cell
          :text="`Maakt ${en.regulatory_layer || '?'} mogelijk`"
          :supporting-text="en.subject || ''"
        ></ndd-text-cell>
      </ndd-list-item>

      <!-- Inbound: implementors -->
      <ndd-list-item v-for="h in implementors" :key="`by-impl-${h.law_id}-${h.article}-${h.open_term}`" size="md">
        <ndd-text-cell
          :text="`Open term wordt ingevuld door '${h.open_term || '—'}'`"
        ></ndd-text-cell>
        <ndd-cell>
          <ndd-button
            size="md"
            :text="`${h.law_id} art ${h.article}`"
            @click="goTo(h.law_id, h.article)"
          ></ndd-button>
        </ndd-cell>
      </ndd-list-item>

      <!-- Inbound: overriders -->
      <ndd-list-item v-for="h in overriders" :key="`by-ov-${h.law_id}-${h.article}-${h.target_output}`" size="md">
        <ndd-text-cell :text="`Output '${h.target_output}' wordt overschreven`"></ndd-text-cell>
        <ndd-cell>
          <ndd-button
            size="md"
            :text="`${h.law_id} art ${h.article}`"
            @click="goTo(h.law_id, h.article)"
          ></ndd-button>
        </ndd-cell>
      </ndd-list-item>

      <!-- Inbound: users -->
      <ndd-list-item v-for="h in users" :key="`by-use-${h.law_id}-${h.article}-${h.input_name}`" size="md">
        <ndd-text-cell
          :text="`Output '${h.target_output || '?'}' wordt gebruikt`"
          :supporting-text="`als input '${h.input_name}'`"
        ></ndd-text-cell>
        <ndd-cell>
          <ndd-button
            size="md"
            :text="`${h.law_id} art ${h.article}`"
            @click="goTo(h.law_id, h.article)"
          ></ndd-button>
        </ndd-cell>
      </ndd-list-item>

      <ndd-list-item v-if="reverseLoading && !implementors.length && !overriders.length && !users.length" size="md">
        <ndd-text-cell text="Relaties zoeken in corpus…"></ndd-text-cell>
      </ndd-list-item>
    </ndd-list>
    <ndd-spacer size="16"></ndd-spacer>
  </ndd-simple-section>
</template>

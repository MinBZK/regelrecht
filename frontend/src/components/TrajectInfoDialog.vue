<script setup>
import { computed, nextTick, ref, watch } from 'vue';
import {
  useTrajectDetail,
  writableSource,
  branchTreeUrl,
} from '../composables/useTrajectDetail.js';

const props = defineProps({
  /** Whether the sheet is currently open. */
  modelValue: { type: Boolean, default: false },
  /** Traject to show (UUID id, same value the members dialog takes). */
  trajectId: { type: String, default: null },
  /** Traject display name, for the sheet header. */
  trajectName: { type: String, default: '' },
});

const emit = defineEmits(['update:modelValue']);

const sheetEl = ref(null);
const { detail, loading, error: loadError, load } = useTrajectDetail();

// Repo/branch come from the writable-own source; null-safe so an unexpected
// shape renders "onbekend" instead of crashing.
const source = computed(() => writableSource(detail.value));
const repoLabel = computed(() =>
  source.value ? `${source.value.gh_owner}/${source.value.gh_repo}` : null,
);
const repoUrl = computed(() => branchTreeUrl(source.value));
const subpath = computed(() => {
  const p = source.value?.gh_path;
  return p && p.trim() ? p : 'repo-root';
});

// dash for empty optional text fields.
function orDash(v) {
  return v && String(v).trim() ? v : '—';
}

watch(
  () => props.modelValue,
  async (v) => {
    if (v) {
      // Kick off the fetch before awaiting nextTick so the request settles
      // promptly (the test drives this with a couple of nextTick flushes).
      const loaded = props.trajectId ? load(props.trajectId) : Promise.resolve();
      await nextTick();
      sheetEl.value?.show();
      await loaded;
    } else {
      await nextTick();
      sheetEl.value?.hide();
    }
  },
);

function close() {
  emit('update:modelValue', false);
}

defineExpose({ close });
</script>

<template>
  <!--
    NOTE: TrajectMembersDialog wraps this sheet in <Teleport to="body">. We
    deliberately do not, because Vue Test Utils 2.4.10 cannot reach
    teleported content from the wrapper (no teleport stub is configured in
    this project), and the spec's tests assert on wrapper.text()/wrapper.get().
    nldd-sheet handles its own overlay/portaling at runtime, so dropping the
    Teleport has no behavioural effect on the rendered sheet.
  -->
  <nldd-sheet
    ref="sheetEl"
    placement="right"
    width="520px"
    full-height
    @close="close"
  >
      <nldd-page sticky-header sticky-footer>
        <nldd-top-title-bar
          slot="header"
          :text="`Traject-info — ${trajectName}`"
          dismiss-text="Sluit"
          @dismiss="close"
        ></nldd-top-title-bar>

        <nldd-simple-section v-if="loading">
          <p class="traject-info-status">Laden…</p>
        </nldd-simple-section>

        <nldd-simple-section v-else-if="loadError">
          <p class="traject-info-error">
            {{ loadError.message || 'Fout bij laden' }}
          </p>
        </nldd-simple-section>

        <template v-else-if="detail">
          <nldd-simple-section heading="Gegevens">
            <nldd-list variant="box" class="traject-info-list">
              <nldd-list-item size="md">
                <nldd-text-cell text="Naam" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ detail.name }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Beschrijving" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ orDash(detail.description) }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Scope" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ orDash(detail.scope) }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Status" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ detail.status }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Jouw rol" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ detail.role }}</span></nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-simple-section>

          <nldd-simple-section heading="Repository">
            <nldd-list variant="box" class="traject-info-list">
              <nldd-list-item size="md">
                <nldd-text-cell
                  text="Repo"
                  supporting-text="Opent de traject-branch op GitHub in een nieuw tabblad."
                  max-width="180px"
                ></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell>
                  <!-- nldd-link is the design-system link component. It
                       auto-sets rel='noopener noreferrer' for target='_blank',
                       but we also pass rel explicitly so it is present even
                       before the Lit component upgrades (and is unit-testable).
                       end-icon hints the link leaves the app. -->
                  <nldd-link
                    v-if="repoUrl"
                    class="traject-info-repo-link"
                    size="md"
                    :href="repoUrl"
                    target="_blank"
                    rel="noopener noreferrer"
                    end-icon="external-link"
                    :text="repoLabel"
                  ></nldd-link>
                  <span v-else class="traject-info-value">{{ repoLabel || 'onbekend' }}</span>
                </nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Branch" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ source?.gh_branch || 'onbekend' }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Base branch" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ source?.gh_base_branch || 'onbekend' }}</span></nldd-cell>
              </nldd-list-item>
              <nldd-list-item size="md">
                <nldd-text-cell text="Subpath" max-width="180px"></nldd-text-cell>
                <nldd-spacer-cell size="8"></nldd-spacer-cell>
                <nldd-cell><span class="traject-info-value">{{ subpath }}</span></nldd-cell>
              </nldd-list-item>
            </nldd-list>
          </nldd-simple-section>
        </template>

        <nldd-container slot="footer" padding="16">
          <nldd-button
            variant="ghost"
            size="md"
            full-width
            text="Sluiten"
            @click="close"
          ></nldd-button>
        </nldd-container>
      </nldd-page>
  </nldd-sheet>
</template>

<style scoped>
.traject-info-list nldd-cell {
  flex: 1;
  min-width: 0;
}
.traject-info-value {
  font-size: 14px;
  word-break: break-word;
}
.traject-info-status {
  font-size: 13px;
  color: var(--semantics-content-secondary-color, #555);
  margin: 8px 0;
}
.traject-info-error {
  color: var(--nldd-color-text-error, #c62828);
  font-size: 13px;
  margin-top: 8px;
}
.traject-info-repo-link {
  word-break: break-word;
}
</style>
